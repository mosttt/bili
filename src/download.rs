use std::path::{Path};
use bilirust::{Audio, FNVAL_DASH, FNVAL_MP4, Video, VIDEO_QUALITY_4K};
use lazy_static::lazy_static;
use crate::{local, user};
use console::{Emoji};
use dialoguer::Select;

lazy_static! {
     static ref BV_PATTERN: regex::Regex = regex::Regex::new(r"BV[0-9a-zA-Z]{10}").unwrap();
}

pub(crate) async fn download(url: String) -> crate::Result<()> {
    if let Some(find) = BV_PATTERN.find(url.as_str()) {
        return download_bv((&url[find.start()..find.end()]).to_string()).await;
    }
    Ok(())
}

async fn download_bv(bv: String) -> crate::Result<()> {

    let client = user::login_client().await?;
    println!();
    println!("{}匹配到：{}", Emoji("✨", ":-)"), bv.as_str());
    println!();
    let bv_info = client.bv_info(bv.clone()).await.unwrap();
    println!(" {}", bv_info.title.as_str());
    println!();

    let video_format = choose_video_format();
    let format_value = video_format_parameters(video_format);
    let video_url = client.bv_download_url(bv.clone(), bv_info.cid, format_value, VIDEO_QUALITY_4K).await?;
    match video_format {
        "dash" => {
            let support_formats = &video_url.support_formats;
            if support_formats.len() == 0 { panic!("未找到！") }

            //视频
            let mut choose_string = vec![];
            let mut choose_int = vec![];
            for i in 0..video_url.support_formats.len() {
                let f = &support_formats[i];
                choose_string.push(f.new_description.clone());
                choose_int.push(f.quality);
            }
            let quality_video = choose_int[Select::new()
                .with_prompt("选择视频质量")
                .default(0)
                .items(&choose_string)
                .interact()
                .unwrap()];

            // 音频
            let mut choose_string: Vec<String> = vec![];
            let mut choose_int: Vec<i64> = vec![];
            for i in 0..video_url.dash.audio.len() {
                let f = &video_url.dash.audio[i];
                match f.id {
                    30216 => {
                        choose_string.push("64K".to_owned());
                        choose_int.push(f.id);
                    }
                    30232 => {
                        choose_string.push("132K".to_owned());
                        choose_int.push(f.id);
                    }
                    30280 => {
                        choose_string.push("192K".to_owned());
                        choose_int.push(f.id);
                    }
                    _ => {
                        choose_string.push(format!("AUDIO-{}", f.id));
                        choose_int.push(f.id);
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
            let mut video: Option<Video> = None;
            for x in video_url.dash.video {
                if x.id == quality_video {
                    video = Some(x);
                    break;
                }
            }
            let mut audio: Option<Audio> = None;
            for x in video_url.dash.audio {
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
            let current_exe_directory = local::current_exe_directory();

            let video_filename = format!("{}.video", name);
            let audio_filename = format!("{}.audio", name);

            //下载
            println!("{}开始下载视频...", Emoji("🚚 ", ""));
            down_file_to(&video.base_url, &current_exe_directory, &video_filename).await;
            println!("{}开始下载音频...", Emoji("🚚 ", ""));
            down_file_to(&video.base_url, &current_exe_directory, &audio_filename).await;

            //合并

            //清理数据
        }
        "mp4" => {}
        _ => panic!("e2")
    }
    Ok(())
}

async fn down_file_to(url: &str, path: &Path, filename: &str) {
    println!("url:{} path:{} filename:{}",url,path.display(),filename);
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