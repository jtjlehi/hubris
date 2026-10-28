fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    idol::Generator::new().build_server_support(
        "../../idl/pio.idol",
        "server_stub.rs",
        idol::server::ServerStyle::InOrder,
    )?;

    Ok(())
}
