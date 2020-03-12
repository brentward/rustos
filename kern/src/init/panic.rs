use core::panic::PanicInfo;
use crate::console::kprintln;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    kprintln!(r#"                            .N8                     .,                          "#);
    kprintln!(r#"                             +.7=                   M=.                         "#);
    kprintln!(r#"                            .M  ,M                 M..M.                        "#);
    kprintln!(r#"                             ..   .M              M.  ..                        "#);
    kprintln!(r#"                              $     ,M           M     .                        "#);
    kprintln!(r#"                              M.      8M        M      :                        "#);
    kprintln!(r#"                              .        .M.    .?       .                        "#);
    kprintln!(r#"                              .         .++  M        .                         "#);
    kprintln!(r#"                                          .MM         8.                        "#);
    kprintln!(r#"                               .                      M.        .,              "#);
    kprintln!(r#"                              ,.                      M.. NMN . M               "#);
    kprintln!(r#"                N...      . . M.                               M                "#);
    kprintln!(r#"                ..                    .M...7M . . NI.         M                 "#);
    kprintln!(r#"                 .                 .M  MI            M       :.                 "#);
    kprintln!(r#"                  ..              D.,,   ..MMOOMMM.   .     .M                  "#);
    kprintln!(r#"                    .           N ..  .:+            M M    M                   "#);
    kprintln!(r#"                     .         M.M  .N                     +                    "#);
    kprintln!(r#"                     .7.      M.M  +.           . ..   M   =..                  "#);
    kprintln!(r#"                       D     ..Z..?   MM~      .              ~                 "#);
    kprintln!(r#"                        M    Z    ...8~   D     . 7MM   .        IM.            "#);
    kprintln!(r#"                  ...,M      M  .M.      8.    I       =M       .M.             "#);
    kprintln!(r#"             +M8,...         M  .         .N  M.        .7     M.               "#);
    kprintln!(r#"            M                M 7           .             $.   ,.                "#);
    kprintln!(r#"             . N             8 .       MM   MN   M:      .. ~.                  "#);
    kprintln!(r#"                .M.          +.             M..          D  N                   "#);
    kprintln!(r#"                  ..M.       ..M           .  8.        .:  .,                  "#);
    kprintln!(r#"                      .M     .  M         $.  . D     .D.D    .M                "#);
    kprintln!(r#"                      M      ..   M.    M,.   M  .. ..  .:      .M.             "#);
    kprintln!(r#"                     Z       ..  :.     M.    . .M   .? .M    DM                "#);
    kprintln!(r#"                  :D         ..    .,...      ..         , MM                   "#);
    kprintln!(r#"                .M.           =             M .         .  D                    "#);
    kprintln!(r#"                    .MM,   D  M              ,..         . MM.                  "#);
    kprintln!(r#"                        .M M   .               .MMMMMM=. MM  .8                 "#);
    kprintln!(r#"                         O  .  =            :O+M:MMMMMMM8 MM.                   "#);
    kprintln!(r#"                        ~    ==M        .ZMMMMMMMM,.M.MMM M.:                   "#);
    kprintln!(r#"                       IMD8Z?=  ..   OM,NMMM 7.N?.       O  .                   "#);
    kprintln!(r#"                              +.MM M..MMMMMM=  M                                "#);
    kprintln!(r#"                                M MMMMMMMM   N         M                        "#);
    kprintln!(r#"                              MM8 MMMMM .M,.M          .                        "#);
    kprintln!(r#"                                M  8. M.   .D       .D                          "#);
    kprintln!(r#"                                 MMMMN    ..M     . O                           "#);
    kprintln!(r#"                                       NZ . .. .M$                              "#);
    kprintln!("┌──────────────────────────────────────────────────────────────────────────────┐");
    kprintln!("│                                !!!  PANIC  !!!                               │");
    kprintln!("└──────────────────────────────────────────────────────────────────────────────┘");
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
