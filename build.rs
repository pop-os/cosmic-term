fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Rebuild if i18n files change
    println!("cargo:rerun-if-changed=i18n");

    vergen::EmitBuilder::builder()
        .fail_on_error()
        .git_commit_date()
        .git_sha(true)
        .emit()?;
    Ok(())
}
