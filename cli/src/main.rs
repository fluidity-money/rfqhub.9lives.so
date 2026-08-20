use std::env::args;

use superposition_librfq::Args;

fn main() {
    let arg1 = args().skip(1).next().take().unwrap();
    println!(
        "0x{}",
        const_hex::encode(borsh::to_vec(&serde_json::from_str::<Args>(&arg1).unwrap()).unwrap())
    )
}
