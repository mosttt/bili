use crate::{cli, ffmpeg, local, user};
use anyhow::{Context, Ok};
use bilirust::{Audio, Ss, SsState, Video, FNVAL_DASH, FNVAL_MP4, VIDEO_QUALITY_4K};
use console::Emoji;
use dialoguer::Select;
use futures::stream::TryStreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use lazy_static::lazy_static;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio_util::io::StreamReader;

lazy_static! {
    static ref SHORT_PATTERN: regex::Regex =
        regex::Regex::new(r"//b\d+\.tv/([0-9a-zA-Z]+)$").unwrap();
    static ref BV_PATTERN: regex::Regex = regex::Regex::new(r"BV[0-9a-zA-Z]{10}").unwrap();
    static ref SERIES_PATTERN: regex::Regex = regex::Regex::new(r"((ep)|(ss))[0-9]+").unwrap();
    static ref USER_COLLECTION_DETAIL_PATTERN: regex::Regex =
        regex::Regex::new(r"/([0-9]+)/channel/collectiondetail\?sid=([0-9]+)").unwrap();
}

pub(crate) async fn download(url: String) -> crate::Result<()> {
    let mut url = url;
    //解析短链接并重定向
    if let Some(_) = SHORT_PATTERN.find(url.as_str()) {
        let rsp = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()?
            .get(&url)
            .send()
            .await?;
        match rsp.status().as_u16() {
            302 => {
                let headers = rsp.headers();
                let location = headers.get("location");
                if let Some(location) = location {
                    url = location.to_str()?.to_owned();
                }
            }
            _ => return Err(anyhow::Error::msg("resolve short links error")),
        }
    }
    //下载bv链接
    if let Some(find) = BV_PATTERN.find(url.as_str()) {
        return download_bv((&url[find.start()..find.end()]).to_string()).await;
    }
    //下载系列 动漫 视频
    if let Some(find) = SERIES_PATTERN.find(url.as_str()) {
        return download_series((&(url[find.start()..find.end()])).to_owned(), url).await;
    }
    //下载用户的合集
    if let Some(find) = USER_COLLECTION_DETAIL_PATTERN.captures(url.as_str()) {
        let mid: i64 = find.get(1).unwrap().as_str().parse().unwrap();
        let sid: i64 = find.get(2).unwrap().as_str().parse().unwrap();
        return download_collection_detail(mid, sid).await;
    }
    Ok(())
}

async fn download_series(id: String, url: String) -> crate::Result<()> {
    let client = user::login_client().await?;

    println!();
    println!("{}匹配到合集 : {}", Emoji("✨", ""), id);

    let ss_state = if cli::parse_input_url_value() {
        client.videos_info_by_url(url).await?
    } else {
        client.videos_info(id.clone()).await?
    };

    println!("  系列名称 : {}", ss_state.media_info.series);
    println!(
        "  包含番剧 : {} ",
        ss_state
            .ss_list
            .iter()
            .map(|i| i.title.as_str())
            .join(" / ")
    );

    let folder = PathBuf::from(local::allowed_file_name(
        ss_state.media_info.series.as_str(),
    ));

    println!("  保存位置 : {}", folder.to_str().unwrap());

    tokio::fs::create_dir_all(folder.as_path()).await?;

    //获得下载的合集id
    let fetch_ids = if cli::choose_seasons_value() {
        let titles: Vec<String> = ss_state
            .ss_list
            .iter()
            .map(|x| format!("{} ({})", x.id, x.title.as_str()))
            .collect();
        let default_selects = vec![true; titles.len()];

        let selects = dialoguer::MultiSelect::new()
            .with_prompt("请选择要下载的合集")
            .items(&titles)
            .defaults(&default_selects)
            .interact()
            .unwrap();

        let mut id_list: Vec<i64> = vec![];

        for i in 0..titles.len() {
            if selects.contains(&i) {
                id_list.push(ss_state.ss_list[i].id);
            }
        }
        id_list
    } else {
        ss_state.ss_list.iter().map(|x| x.id).collect()
    };

    // 找到所有的ss
    // 找到所有ss的bv
    println!();
    println!("搜索视频");
    let mut sss: Vec<(Ss, SsState, String)> = vec![];
    for x in ss_state.ss_list {
        if !fetch_ids.contains(&x.id) {
            continue;
        }
        let videos_info = client.videos_info(format!("ss{}", x.id)).await.unwrap();
        let x_dir_name = format!(
            "{} ({}) {}",
            x.id,
            x.title.as_str(),
            videos_info.media_info.season_title.as_str(),
        );
        println!(
            "  {} : 共 {} 个视频",
            x_dir_name.as_str(),
            videos_info.ep_list.len()
        );
        sss.push((x, videos_info, x_dir_name));
    }
    println!();
    println!("下载视频");
    for x in &sss {
        let ss_folder = folder.join(x.2.as_str());
        std::fs::create_dir_all(ss_folder.as_path()).unwrap();

        for ep in &x.1.ep_list {
            let name = format!("{}. ({}) {}", ep.i, ep.title_format, ep.long_title);
            let name = local::allowed_file_name(&name);
            println!();
            println!("{}", name);
            let audio_file = ss_folder.join(format!("{}.audio", name));
            let video_file = ss_folder.join(format!("{}.video", name));
            let mix_file = ss_folder.join(format!("{}.mp4", name));
            if mix_file.exists() {
                println!("已存在：{}", name);
                continue;
            }
            let media_url = client
                .bv_download_url(
                    ep.bvid.clone(),
                    ep.cid.clone(),
                    FNVAL_DASH,
                    VIDEO_QUALITY_4K,
                )
                .await?;
            let audio_url = media_url.dash.audio.first().unwrap().base_url.as_str();
            let video_url = media_url.dash.video.first().unwrap().base_url.as_str();
            //下载
            down_file_to(video_url, &video_file, "下载视频").await;
            println!("{}下载视频完成", Emoji("🚚 ", ""));
            down_file_to(audio_url, &audio_file, "下载音频").await;
            println!("{}下载音频完成", Emoji("🚚 ", ""));

            println!("开始合并视频：{}", format!("{}.mp4", name));
            ffmpeg::ffmpeg_merge_file(
                vec![video_file.to_str().unwrap(), audio_file.to_str().unwrap()],
                mix_file.to_str().unwrap(),
            )
            .unwrap();
            println!("{}合并视频完成", Emoji("✨", ""));
            let _ = std::fs::remove_file(&audio_file);
            let _ = std::fs::remove_file(&video_file);
            println!("{}完成数据清理", Emoji("🚚 ", ""));
        }
    }
    println!();
    println!("{}全部完成", Emoji("✨", ""));
    Ok(())
}

async fn download_collection_detail(mid: i64, sid: i64) -> crate::Result<()> {
    let client = user::login_client().await?;
    let mut current_page = 1;
    let mut page_info = client
        .collection_video_page(mid, sid, false, current_page, 20)
        .await?;

    println!();
    println!("{}获取到到合集：{}", Emoji("✨", ""), page_info.meta.name);
    println!();

    let folder = local::allowed_file_name(&page_info.meta.name);
    tokio::fs::create_dir_all(&folder).await?;

    let path = PathBuf::from(&folder);
    loop {
        //下载视频
        for archive in page_info.archives {
            println!();
            println!("开始下载：{}", archive.title);

            let name = local::allowed_file_name(&archive.title);
            let video_file = path.join(format!("{}.video", name));
            let audio_file = path.join(format!("{}.audio", name));
            let mix_file = path.join(format!("{}.mp4", name));

            if mix_file.exists() {
                println!("已存在：{}", archive.title);
                continue;
            }

            let bv_info = client.bv_info(archive.bvid).await?;
            let media_url = client
                .bv_download_url(bv_info.bvid, bv_info.cid, FNVAL_DASH, VIDEO_QUALITY_4K)
                .await?;

            let video_url = media_url.dash.video.first().unwrap().base_url.as_str();
            let audio_url = media_url.dash.audio.first().unwrap().base_url.as_str();

            //下载
            down_file_to(video_url, &video_file, "下载视频").await;
            println!("{}下载视频完成", Emoji("🚚 ", ""));

            down_file_to(audio_url, &audio_file, "下载音频").await;
            println!("{}下载音频完成", Emoji("🚚 ", ""));

            println!("开始合并视频：{}", format!("{}.mp4", name));
            ffmpeg::ffmpeg_merge_file(
                vec![video_file.to_str().unwrap(), audio_file.to_str().unwrap()],
                mix_file.to_str().unwrap(),
            )
            .unwrap();
            println!("{}合并视频完成", Emoji("✨", ""));
            let _ = std::fs::remove_file(&audio_file);
            let _ = std::fs::remove_file(&video_file);
            println!("{}完成数据清理", Emoji("🚚 ", ""));
        }
        // 获取下一页
        if page_info.page.page_size * page_info.page.page_num >= page_info.page.total {
            break;
        }
        current_page += 1;
        page_info = client
            .collection_video_page(mid, sid, false, current_page, 20)
            .await?;
    }
    println!();
    println!("{}全部完成", Emoji("✨", ""));
    Ok(())
}

async fn download_bv(bv: String) -> crate::Result<()> {
    let client = user::login_client().await?;
    println!();
    println!("{}匹配到：{}", Emoji("✨", ""), bv.as_str());
    println!();
    let bv_info = client.bv_info(bv.clone()).await.unwrap();
    println!(" {}", bv_info.title.as_str());
    println!();

    let video_format = choose_video_format();
    let format_value = video_format_parameters(video_format);
    let media_url = client
        .bv_download_url(bv.clone(), bv_info.cid, format_value, VIDEO_QUALITY_4K)
        .await?;
    match video_format {
        "dash" => {
            if media_url.support_formats.len() == 0 {
                panic!("未找到！")
            }

            //视频
            let mut choose_string = vec![];
            let mut choose_int = vec![];
            for v in &media_url.dash.video {
                if !choose_int.contains(&v.id) {
                    choose_int.push(v.id);
                    match v.id {
                        120 => choose_string.push("4K".to_string()),
                        116 => choose_string.push("1080P 60".to_string()),
                        80 => choose_string.push("1080P".to_string()),
                        64 => choose_string.push("720P".to_string()),
                        32 => choose_string.push("480P".to_string()),
                        16 => choose_string.push("360P".to_string()),
                        _ => choose_string.push(format!("VEDIO-{}", v.id)),
                    }
                }
            }
            let quality_video = choose_int[Select::new()
                .with_prompt("选择视频质量")
                .default(0)
                .items(&choose_string)
                .interact()
                .unwrap()];

            // 音频
            let mut choose_string = vec![];
            let mut choose_int = vec![];
            for a in &media_url.dash.audio {
                if !choose_int.contains(&a.id) {
                    choose_int.push(a.id);
                    match a.id {
                        30216 => choose_string.push("64K".to_string()),
                        30232 => choose_string.push("132K".to_string()),
                        30280 => choose_string.push("192K".to_string()),
                        _ => choose_string.push(format!("AUDIO-{}", a.id)),
                    }
                }
            }
            let quality_audio = choose_int[Select::new()
                .with_prompt("选择音频质量")
                .default(0)
                .items(&choose_string)
                .interact()
                .unwrap()];

            // 下载
            let mut video: Option<&Video> = None;
            for x in &media_url.dash.video {
                if x.id == quality_video {
                    video = Some(x);
                    break;
                }
            }
            let mut audio: Option<&Audio> = None;
            for x in &media_url.dash.audio {
                if x.id == quality_audio {
                    audio = Some(x);
                    break;
                }
            }

            //没找到应该重新来过 递归
            let video = video.unwrap();
            let audio = audio.unwrap();

            //构建路径
            let name = local::allowed_file_name(&bv_info.title);

            let video_file = PathBuf::from(format!("{}.video", name));
            let audio_file = PathBuf::from(format!("{}.audio", name));
            let mix_file = PathBuf::from(format!("{}.mp4", name));

            println!("{}下载到文件 : {}", Emoji("✨", ""), mix_file.display());
            if mix_file.exists() {
                panic!("文件已存在");
            }

            //下载
            down_file_to(&video.base_url, &video_file, "下载视频").await;
            println!("{}下载视频完成", Emoji("🚚 ", ""));

            down_file_to(&audio.base_url, &audio_file, "下载音频").await;
            println!("{}下载音频完成", Emoji("🚚 ", ""));

            println!("开始合并视频：{}", format!("{}.mp4", name));
            ffmpeg::ffmpeg_merge_file(
                vec![video_file.to_str().unwrap(), audio_file.to_str().unwrap()],
                mix_file.to_str().unwrap(),
            )
            .unwrap();
            println!("{}合并视频完成", Emoji("✨", ""));
            let _ = std::fs::remove_file(&audio_file);
            let _ = std::fs::remove_file(&video_file);
            println!("{}完成数据清理", Emoji("🚚 ", ""));
        }
        "mp4" => {
            let name = local::allowed_file_name(&bv_info.title);
                let mp4_file = PathBuf::from(format!("{}.mp4", name));
                println!("下载到文件 : {}", mp4_file.display());
                if mp4_file.exists() {
                    panic!("文件已存在");
                }
                down_file_to(&media_url.durl.first().unwrap().url, &mp4_file, "下载中").await;
                println!("下载完成");
        }
        _ => panic!("e2"),
    }
    Ok(())
}

async fn down_file_to(url: &str, file: &Path, title: &str) {
    let checkpoint = if cli::resume_download_value() && file.exists() {
        file.metadata().unwrap().len()
    } else {
        0
    };
    let rsp = request_resource(url).await;
    let size = content_length(&rsp).unwrap();

    let (rsp, file) = if checkpoint == 0 {
        (rsp, tokio::fs::File::create(&file).await.unwrap())
    } else {
        if size == checkpoint {
            return;
        }
        drop(rsp);
        (
            request_resource_rang(url, checkpoint).await,
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(file)
                .await
                .unwrap(),
        )
    };
    let mut writer = BufWriter::with_capacity(1 << 18, file);
    let mut buffer = Box::new([0; 1 << 18]);
    let mut reader = BufReader::with_capacity(
        1 << 18,
        StreamReader::new(
            rsp.bytes_stream()
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err)),
        ),
    );

    let (sender, mut receiver) = tokio::sync::mpsc::channel(1 << 10);

    let sjb = tokio::spawn(async move {
        loop {
            let read = reader.read(buffer.as_mut()).await.unwrap();
            if read == 0 {
                break;
            }
            sender.send(buffer[0..read].to_vec()).await.unwrap();
        }
    });

    let title = title.to_string();
    let rjb = tokio::spawn(async move {
        let pb = ProgressBar::new(size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    &*("".to_owned()
                        + "{spinner:.green}  "
                        + title.as_str()
                        + " [{wide_bar:.cyan/blue}] {bytes}/{total_bytes}"),
                )
                .unwrap()
                .progress_chars("#>-"),
        );
        let mut download_count = checkpoint;
        pb.set_position(download_count);
        while let Some(msg) = receiver.recv().await {
            writer.write_all(&msg).await.unwrap();
            download_count += msg.len() as u64;
            pb.set_position(download_count);
        }
        pb.finish_and_clear();
        writer.flush().await.unwrap();
    });
    //     let (s,r) = tokio::join!(rjb,sjb);
    //    s.unwrap();
    //    r.unwrap();
    sjb.await.unwrap();
    rjb.await.unwrap();
}

fn choose_video_format() -> &'static str {
    ["dash", "mp4"][Select::new()
        .with_prompt("选择视频格式")
        .default(0)
        .items(&["dash (高清)", "mp4 (低清)"])
        .interact()
        .unwrap()]
}

fn video_format_parameters(format_str: &str) -> i64 {
    match format_str {
        "mp4" => FNVAL_MP4,
        "dash" => FNVAL_DASH,
        _ => panic!("格式不正确"),
    }
}

async fn request_resource(url: &str) -> reqwest::Response {
    reqwest::Client::new().get(url).header(
        "user-agent",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/98.0.4758.80 Safari/537.36",
    ).header("referer", "https://www.bilibili.com").send().await.unwrap().error_for_status().unwrap()
}

async fn request_resource_rang(url: &str, begin: u64) -> reqwest::Response {
    reqwest::Client::new().get(url).header(
        "user-agent",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/98.0.4758.80 Safari/537.36",
    ).header("referer", "https://www.bilibili.com").header("Range",format!("bytes={}-",begin)).send().await.unwrap().error_for_status().unwrap()
}

fn content_length(rsp: &reqwest::Response) -> crate::Result<u64> {
    Ok(rsp
        .headers()
        .get("content-length")
        .with_context(|| "未能取得文件长度, HEADER不存在")?
        .to_str()
        .with_context(|| "未能取得文件长度, HEADER不能使用")?
        .parse()
        .with_context(|| "未能取得文件长度, HEADER不能识别未数字")?)
}
