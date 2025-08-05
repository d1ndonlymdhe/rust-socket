use cursive::views::{Dialog, TextView};

pub fn client(){
    // let server_stream = TcpStream::connect("127.0.0.1:8000").expect("Could not connect to server");
    let mut siv = cursive::default();
    siv.add_layer(Dialog::around(TextView::new("Client is running..."))
        .title("Client")
        .button("Quit", |s| s.quit()));
    siv.run();
}