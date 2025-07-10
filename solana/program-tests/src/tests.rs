use crate::setup::init_env;

#[tokio::test]
async fn test_anything() {
    let (bank, payer) = init_env().await;
    println!("{:?}", payer);
    assert!(true);
}
