//! No secrets are printed. Configure private environment variables, not command-line arguments.
use kdpika_licensing::{Environment, LicenseClient};
fn main() {
    if run().is_err() {
        eprintln!("Check your application settings and private license storage.");
        std::process::exit(2);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let app = std::env::var("KDP_APP_ID")?;
    let state = std::env::var("KDP_STATE_DIR")?;
    let client = LicenseClient::builder(app)
        .environment(Environment::Sandbox)
        .storage_dir(state)
        .application_name("KDPika Rust example")
        .application_version("1.0.0")
        .build()?;
    let action = std::env::args().nth(1).unwrap_or_else(|| "check".into());
    let result = match action.as_str() {
        "activate" => client.activate(
            &std::env::var("KDP_LICENSE_KEY")?,
            std::env::var("KDP_CUSTOMER_EMAIL").ok().as_deref(),
        ),
        "check" => client.check(),
        "validate" => client.validate(),
        "online" => client.validate_online(),
        "deactivate" => client.deactivate(),
        _ => return Err("Choose activate, check, validate, online or deactivate".into()),
    };
    println!("{}", result.message());
    if let Some(id) = result.request_id() {
        println!("Support reference: {id}");
    }
    if result.valid() && result.feature("pdf_export") {
        println!("PDF export is included.");
    }
    if !result.valid() && !result.deactivated() {
        std::process::exit(1);
    }
    Ok(())
}
