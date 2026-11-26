use std::char;


fn main() {
    let number = (0u32..0x10FFFF)
        .filter(|x| char::from_u32(*x).is_some_and(|c| unicode_names2::name(c).is_some()))
        .count();

    println!("there are {} named code points", number)
}
