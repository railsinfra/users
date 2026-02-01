fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_client(true)
        .compile_protos(&["proto/accounts.proto"], &["proto"])?;
    Ok(())
}
