use std::env::args;

use superposition_librfq::Args;

fn main() {
    let arg1 = args().nth(1).unwrap();
    println!(
        "0x{}",
        const_hex::encode(borsh::to_vec(&serde_json::from_str::<Args>(&arg1).unwrap()).unwrap())
    )
}
