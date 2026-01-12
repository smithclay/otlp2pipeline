use anyhow::{bail, Result};
use std::io::{self, Write};
use std::process::Command;

use crate::cli::auth;
use crate::cli::commands::naming::bucket_name;
use crate::cli::BucketDeleteArgs;
use crate::cloudflare::CloudflareClient;

pub async fn execute_bucket_delete(args: BucketDeleteArgs) -> Result<()> {
    let bucket = args.bucket.unwrap_or_else(|| bucket_name(&args.name));

    eprintln!("==> Deleting all objects in bucket: {}", bucket);

    // Resolve auth to get account ID
    let creds = auth::resolve_credentials()?;
    let client = CloudflareClient::new(creds.token, creds.account_id).await?;
    let account_id = client.account_id();

    let endpoint = format!("https://{}.r2.cloudflarestorage.com", account_id);
    eprintln!("    Endpoint: {}", endpoint);

    // Check for aws cli
    if Command::new("aws").arg("--version").output().is_err() {
        bail!(
            "aws CLI not found\n\n\
            Install AWS CLI:\n  \
            brew install awscli\n  \
            # or see https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html"
        );
    }

    // Confirmation prompt
    if !args.force {
        eprint!(
            "\nThis will DELETE ALL OBJECTS in s3://{}. Continue? [y/N] ",
            bucket
        );
        io::stderr().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            eprintln!("Aborted.");
            return Ok(());
        }
    }

    eprintln!("\n==> Running: aws s3 rm s3://{}/ --recursive", bucket);

    let status = Command::new("aws")
        .args(["s3", "rm", &format!("s3://{}/", bucket), "--recursive"])
        .env("AWS_ACCESS_KEY_ID", &args.access_key_id)
        .env("AWS_SECRET_ACCESS_KEY", &args.secret_access_key)
        .env("AWS_ENDPOINT_URL", &endpoint)
        .env("AWS_REGION", "auto")
        .status()?;

    if !status.success() {
        bail!("aws s3 rm failed");
    }

    eprintln!("\n==> Done");
    Ok(())
}
