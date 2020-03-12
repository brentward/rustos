use core::panic::PanicInfo;
use crate::console::kprintln;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    kprintln!(r#"                             __"#);
    kprintln!(r#"                   _ ,___,-'",-=-."#);
    kprintln!(r#"       __,-- _ _,-'_)_  (""`'-._\ `."#);
    kprintln!(r#"    _,'  __ |,' ,-' __)  ,-     /. |"#);
    kprintln!(r#"  ,'_,--'   |     -'  _)/         `\"#);
    kprintln!(r#",','      ,'       ,-'_,`           :"#);
    kprintln!(r#",'     ,-'       ,(,-(              :"#);
    kprintln!(r#"     ,'       ,-' ,    _            ;"#);
    kprintln!(r#"    /        ,-._/`---'            /"#);
    kprintln!(r#"   /        (____)(----. )       ,'"#);
    kprintln!(r#"  /         (      `.__,     /\ /,"#);
    kprintln!(r#" :           ;-.___         /__\/|"#);
    kprintln!(r#" |         ,'      `--.      -,\ |"#);
    kprintln!(r#" :        /            \    .__/"#);
    kprintln!(r#"  \      (__            \    |_"#);
    kprintln!(r#"   \       ,`-, *       /   _|,\"#);
    kprintln!(r#"    \    ,'   `-.     ,'_,-'    \"#);
    kprintln!(r#"   (_\,-'    ,'\")--,'-'       __\"#);
    kprintln!(r#"    \       /  // ,'|      ,--'  `-."#);
    kprintln!(r#"     `-.    `-/ \'  |   _,'         `."#);
    kprintln!(r#"        `-._ /      `--'/             \"#);
    kprintln!(r#"-hrr-      ,'           |              \"#);
    kprintln!(r#"          /             |               \"#);
    kprintln!(r#"       ,-'              |               /"#);
    kprintln!(r#"      /                 |             -'"#);
    kprintln!("┌────────────────────────────────────────┐");
    kprintln!("│             !!!  D'oh  !!!             │");
    kprintln!("└────────────────────────────────────────┘");
    kprintln!("");
    match _info.location() {
        Some(location) => {
            kprintln!("FILE: {}", location.file());
            kprintln!("LINE: {}", location.line());
            kprintln!("COL: {}", location.column());
        }
        None => kprintln!("Panic location cannot be determined"),
    }
    kprintln!("");
    match _info.message() {
        Some(message) => kprintln!("{}", message),
        None => kprintln!("Panic message cannot be determined"),
    }
    loop {}
}
