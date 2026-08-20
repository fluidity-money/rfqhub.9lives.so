use bobcat_sdk::maths::U;

macro_rules! defaults {
    ( $( $name:ident ),+ ) => {
        defaults!(@count 1, $( $name ),+);
    };
    (@count $n:expr, $name:ident, $( $rest:ident ),+) => {
        #[cfg_attr(not(feature = "testing"), allow(unused))]
        pub const $name: U = U::from_u32($n);
        defaults!(@count ($n + 1), $( $rest ),+);
    };
    (@count $n:expr, $name:ident) => {
        #[cfg_attr(not(feature = "testing"), allow(unused))]
        pub const $name: U = U::from_u32($n);
    };
}

defaults! {
    SERVER_PUBLIC_KEY
}
