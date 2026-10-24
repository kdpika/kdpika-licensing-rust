use kdpika_licensing::{AsyncLicenseClient, Environment, LicenseClient};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let builder = LicenseClient::builder(std::env::var("KDP_APP_ID")?)
        .environment(Environment::Sandbox)
        .storage_dir(std::env::var("KDP_STATE_DIR")?);
    let client = AsyncLicenseClient::build(builder).await?;
    let result = client.check().await?;
    println!("{}", result.message());
    if result.valid() && result.feature("pdf_export") {
        println!("PDF export is included.");
    }
    Ok(())
}
