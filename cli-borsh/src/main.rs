use borsh::BorshDeserialize;

use std::{env::args, io::stdin};

fn main() {
    let args = args().skip(1).collect::<Vec<_>>();
    let (json, should_decode, is_acc) = match args.as_slice() {
        [flag] if flag == "-d" => (None, true, false),
        [flag, is_acc] if flag == "-d" && is_acc == "-a" => (None, true, true),
        [json, flag] if flag == "-d" => (Some(json), true, false),
        [json, flag, is_acc] if flag == "-d" && is_acc == "-a" => (Some(json), true, true),
        [json] => (Some(json), false, false),
        _ => (None, false, false),
    };
    if is_acc {
        if should_decode {
            let x = if let Some(json) = json {
                const_hex::decode(&json).unwrap()
            } else {
                let mut s = String::new();
                stdin().read_line(&mut s).unwrap();
                const_hex::decode(s).unwrap()
            };
            println!(
                "{}",
                superposition_libaccounts::Args::deserialize(&mut x.as_slice()).unwrap()
            )
        } else {
            println!(
                "0x{}",
                const_hex::encode(
                    borsh::to_vec(&if let Some(json) = json {
                        serde_json::from_str::<superposition_libaccounts::Args>(&json).unwrap()
                    } else {
                        serde_json::from_reader::<_, _>(stdin()).unwrap()
                    })
                    .unwrap()
                )
            )
        }
    } else {
        if should_decode {
            let x = if let Some(json) = json {
                const_hex::decode(&json).unwrap()
            } else {
                let mut s = String::new();
                stdin().read_line(&mut s).unwrap();
                const_hex::decode(s).unwrap()
            };
            println!(
                "{}",
                superposition_librfq::Args::deserialize(&mut x.as_slice()).unwrap()
            )
        } else {
            println!(
                "0x{}",
                const_hex::encode(
                    borsh::to_vec(&if let Some(json) = json {
                        serde_json::from_str::<superposition_librfq::Args>(&json).unwrap()
                    } else {
                        serde_json::from_reader::<_, _>(stdin()).unwrap()
                    })
                    .unwrap()
                )
            )
        }
    }
}
