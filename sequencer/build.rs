fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/sequencer.proto");
    std::fs::create_dir_all("src/pb")?;
    tonic_build::configure()
        .out_dir("src/pb")
        .compile_protos(&["proto/sequencer.proto"], &["proto"])?;
    Ok(())
}