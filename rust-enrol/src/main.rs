use clap::Parser;

use pretty_env_logger;
use pretty_env_logger::env_logger::Env;
#[macro_use]
extern crate log;

use reqwest;
use reqwest::header::HeaderMap;
use reqwest::header::AUTHORIZATION;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;

use std::collections::HashMap;
use std::fs;

#[derive(Deserialize, Debug)]
struct Settings {
    region: String,
    img_src: String,
    img_path: String,
    sp_key: String,
    sp_secret: String,
    oa_username: String,
    oa_pw: String,
}

impl Settings {
    fn from_env() -> Self {
        dotenv::dotenv().ok();
        Self {
            region: std::env::var("REGION").unwrap(),
            img_src: std::env::var("IMAGE_SOURCE").unwrap(),
            img_path: std::env::var("IMAGE_PATH").unwrap(),
            sp_key: std::env::var("SP_KEY").unwrap(),
            sp_secret: std::env::var("SP_SECRET").unwrap(),
            oa_username: std::env::var("OAUTH_USERNAME").unwrap(),
            oa_pw: std::env::var("OAUTH_PW").unwrap(),
        }
    }
}

fn request_log(res: reqwest::blocking::Response, msg: &str) -> reqwest::blocking::Response {
    match res.status() {
        StatusCode::OK => {
            info!("{} succeeded", msg);
            res
        }
        status => {
            let err: serde_json::Value = res.json().unwrap();
            if status.is_client_error() {
                error!("Client Error during {:?}: <{}, {}>", msg, status, err);
                std::process::exit(1);
            } else if status.is_server_error() {
                error!("Server Error during {:?}: <{}, {}>", msg, status, err);
                std::process::exit(1);
            } else {
                error!("Unknown Error during {:?}: <{}, {}>", msg, status, err);
                std::process::exit(1);
            }
        }
    }
}

fn create_token(client: &reqwest::blocking::Client, config: &Settings, username: &str) -> String {
    let url = format!(
        "https://{}.secure.iproov.me/api/v2/claim/enrol/token",
        config.region
    );
    let body = json!({
        "resource": "photo_enrol_test",
        "api_key": config.sp_key,
        "secret": config.sp_secret,
        "user_id": username,
    });
    debug!("getting enrol token, url={}, body={:?}", url, body);
    let res = client.post(&url).json(&body).send().unwrap();
    let res: serde_json::Value = request_log(res, "create token").json().unwrap();
    res["token"].as_str().unwrap().to_string()
}

fn send_photo(client: &reqwest::blocking::Client, config: &Settings, token: &str) {
    let image = fs::read(&config.img_path).unwrap();

    let enrol_image_url = format!(
        "https://{}.secure.iproov.me/api/v2/claim/enrol/image",
        config.region
    );

    let multipart = reqwest::blocking::multipart::Form::new()
        .text("api_key", config.sp_key.clone())
        .text("secret", config.sp_secret.clone())
        .text("rotation", "0".to_string())
        .part(
            "image",
            reqwest::blocking::multipart::Part::bytes(image).file_name("image.jpg"),
        )
        .text("token", token.to_string())
        .text("source", config.img_src.clone());

    debug!("sending image for enrolment, url={}", enrol_image_url);
    let res = client
        .post(&enrol_image_url)
        .multipart(multipart)
        .send()
        .unwrap();
    request_log(res, "enrol image");
}

fn create_access_token(client: &reqwest::blocking::Client, config: &Settings) -> String {
    let url = format!(
        "https://{}.secure.iproov.me/api/v2/{}/access_token",
        config.region, config.sp_key
    );

    let mut body = HashMap::new();
    body.insert("grant_type", "client_credentials");

    debug!("getting oauth access token");
    let res = client
        .post(&url)
        .basic_auth(&config.oa_username, Some(&config.oa_pw))
        .form(&body)
        .send()
        .unwrap();

    let json: serde_json::Value = request_log(res, "generate access token").json().unwrap();

    json["access_token"].as_str().unwrap().to_string()
}

fn delete_user(
    client: &reqwest::blocking::Client,
    config: &Settings,
    access_token: &str,
    username: &str,
) {
    let url = format!(
        "https://{}.secure.iproov.me/api/v2/users/{}",
        config.region, username
    );

    debug!("deleting user");
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        format!("Bearer {}", access_token).parse().unwrap(),
    );

    let res = client.delete(&url).headers(headers).send().unwrap();
    request_log(res, "delete user");
    info!("user '{}' deleted", &username)
}

fn photo_enrol(args: &Args, config: &Settings) {
    let username = petname::petname(5, "_");
    static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);
    let client = reqwest::blocking::Client::builder()
        .user_agent(APP_USER_AGENT)
        .build()
        .unwrap();
    let token = create_token(&client, &config, &username);
    send_photo(&client, &config, &token);
    info!("user '{}' enrolled", &username);
    if args.delete_user {
        let access_token = create_access_token(&client, &config);
        delete_user(&client, &config, &access_token, &username);
    }
}

/// simple program to photo enrol
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    /// deletes the user after enrolment
    delete_user: bool,
}

fn main() {
    let args = Args::parse();
    let settings = Settings::from_env();
    pretty_env_logger::env_logger::init_from_env(Env::default().filter_or("LOG_LEVEL", "info"));
    photo_enrol(&args, &settings);
}
