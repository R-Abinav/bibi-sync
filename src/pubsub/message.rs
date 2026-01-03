pub trait Message: Clone + Default + Send + 'static{}

//blanket impl for all types that meet constraints
impl<T: Clone + Default + Send + 'static> Message for T{}

#[cfg(test)]
mod tests{
    use super::*;

    #[derive(Clone, Default)]
    struct TestMsg{
        x: f32, 
        y: f32
    }

    #[test]
    fn test_message_trait_imp(){
        fn accepts_message<T: Message>(_: T){}

        accepts_message(0i32);
        accepts_message(0.0f64);
        accepts_message(true);
        accepts_message(TestMsg{ x: 1.0, y: 2.0 });
    }
}