use qrcode::QrCode;
use tokio::time;
use console::{Emoji, style};
use std::process::exit;
use std::time::Duration;
use bilirust::{from_str, WebToken};
use image::Luma;
use crate::local;


pub(crate) async fn login(is_console: &bool) -> crate::Result<()> {
    let client = bilirust::Client::new();
    let qr_data = client.login_qr().await.unwrap();

    if *is_console {
        qr2term::print_qr(qr_data.url.clone().as_str()).unwrap();
    } else {
        let code = QrCode::new(qr_data.url.clone().as_str().as_bytes()).unwrap();
        let image = code.render::<Luma<u8>>().build();
        let mut path = crate::local::current_exe_directory();
        path.push("qr.png");
        image.save(path.as_path()).unwrap();
        opener::open(path.as_os_str()).unwrap();
    }
    println!("{}等待扫码中...", Emoji("🚚 ", ""));

    loop {
        time::sleep(Duration::from_secs(3)).await;
        let info = client.login_qr_info(qr_data.oauth_key.clone()).await;
        match info {
            Ok(info) => {
                // -1：密钥错误
                // -2：密钥超时
                // -4：未扫描
                // -5：未确认
                match info.error_data {
                    0 => {
                        let web_token = client.login_qr_info_parse_token(info.url).unwrap();
                        let web_token_string = serde_json::to_string(&web_token).unwrap();
                        local::save_property("web_token".to_string(), web_token_string).await?;
                        println!("{}登陆成功！", Emoji("✨", ":-)"));
                        break;
                    }
                    -4 => continue,
                    -5 => continue,
                    -2 => panic!("time out"),
                    other => panic!("ERROR : {}", other),
                }
            }
            Err(err) => {
                panic!("{}", err);
            }
        }
    }

    Ok(())
}

pub(crate) async fn login_client() -> crate::Result<bilirust::Client> {
    let property = local::load_property("web_token".to_owned()).await?;
    if &property == "" {
        println!("{}", style("需要登录!").cyan().bold());
        exit(1);
    }
    let token: WebToken = from_str(property.as_str())?;
    let mut client = bilirust::Client::new();
    client.login_set_sess_data(token.sessdata);
    Ok(client)
}

pub(crate) async fn user_info() -> crate::Result<()> {
    println!("{:?}", login_client().await?.my_info().await?);
    Ok(())
}