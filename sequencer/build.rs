// sequencer/build.rs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/sequencer.proto");
    tonic_build::compile_protos("proto/sequencer.proto")?;
    Ok(())
}