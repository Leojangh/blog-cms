use aws_sdk_s3::Client;
use std::sync::LazyLock;

pub static R2_BUCKET: LazyLock<String> = LazyLock::new(|| {
    std::env::var("R2_BUCKET").expect("R2_BUCKET required")
});
pub static R2_PUBLIC_URL: LazyLock<String> = LazyLock::new(|| {
    std::env::var("R2_PUBLIC_URL").expect("R2_PUBLIC_URL required")
});

pub async fn client() -> Client {
    let endpoint = std::env::var("R2_ENDPOINT").expect("R2_ENDPOINT required");
    let access_key = std::env::var("R2_ACCESS_KEY").expect("R2_ACCESS_KEY required");
    let secret_key = std::env::var("R2_SECRET_KEY").expect("R2_SECRET_KEY required");

    let creds = aws_sdk_s3::config::Credentials::new(access_key, secret_key, None, None, "env");
    let config = aws_sdk_s3::Config::builder()
        .endpoint_url(endpoint)
        .credentials_provider(creds)
        .region(aws_sdk_s3::config::Region::new("auto"))
        .force_path_style(true)
        .behavior_version_latest()
        .build();
    Client::from_conf(config)
}
