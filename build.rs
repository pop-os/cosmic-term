fn main() -> Result<(), Box<dyn std::error::Error>> {
    vergen::EmitBuilder::builder()
        .fail_on_error()
        .git_commit_date()
        .git_sha(true)
        .emit()?;
    Ok(())
}
