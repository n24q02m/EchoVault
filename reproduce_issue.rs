use serde_json::Value;

fn test_func() -> Option<Value> {
    let v = Value::Null;
    v.into()
}

fn main() {
    println!("{:?}", test_func());
}
