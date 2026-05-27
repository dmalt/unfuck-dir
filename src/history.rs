fn hello() {
    println!("hello, world")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        hello();
    }
}
