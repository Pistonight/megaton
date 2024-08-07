#[megaton::bootstrap]
#[module("hello-world")]
#[abort(handler = "my_aobrt", code(-1))]
fn main() {
    // ...
}
