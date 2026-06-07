use std::net::TcpStream;
use std::process::Command;
use std::time::Duration;

/// 启动对话管理 WebUI：检测端口、后台启动服务器、打开浏览器
pub fn launch_webui() {
    let addr = "127.0.0.1:19876";
    let url = "http://127.0.0.1:19876";

    // 检查 WebUI 服务器是否已在运行
    let already_running = TcpStream::connect_timeout(
        &addr.parse().unwrap(),
        Duration::from_millis(500),
    )
    .is_ok();

    if !already_running {
        // 解析用户目录，拼接 server.py 路径
        let home = std::env::var("USERPROFILE").unwrap_or_default();
        let path = home.replace("\\", "/");
        let server_py = format!("{}/.claude/webui/server.py", path);

        // 后台静默启动 Python 服务器
        let _ = Command::new("pythonw")
            .args(["-u", &server_py, "--no-browser"])
            .spawn();

        // 等服务器就绪
        std::thread::sleep(Duration::from_millis(2000));
    }

    // 打开默认浏览器
    let _ = Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn();
}
