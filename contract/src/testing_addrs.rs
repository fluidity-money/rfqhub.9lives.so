use bobcat_sdk::maths::U;

macro_rules! defaults {
    ( $( $name:ident ),+ ) => {
        defaults!(@count 1, $( $name ),+);
    };
    (@count $n:expr, $name:ident, $( $rest:ident ),+) => {
        pub const $name: U = U::from_u32($n);
        defaults!(@count ($n + 1), $( $rest ),+);
    };
    (@count $n:expr, $name:ident) => {
        pub const $name: U = U::from_u32($n);
    };
}

defaults! {
    SERVER_PUBLIC_KEY
}
