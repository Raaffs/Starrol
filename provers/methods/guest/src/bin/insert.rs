use risc0_zkvm::guest::env;

fn main() {
    // TODO: Implement your guest code here

    // read the input
    let mut input: u32 = env::read();
    let adder: u32=env::read();
    // TODO: do something with the input
    input+=adder;
    // write public output to the journal
    env::commit(&input);
}
