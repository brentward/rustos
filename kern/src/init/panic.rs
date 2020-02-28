use core::panic::PanicInfo;
use crate::console::kprintln;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    // kprintln!("          ________");
    // kprintln!("      (( /========\\                                                    _ ._  _ , _ ._");
    // kprintln!("      __/__________\\____________n_                                   (_ ' ( `  )_  .__)");
    // kprintln!("  (( /              \\_____________]                                ( (  (    )   `)  ) _)");
    // kprintln!("    /  =(*)=          \\                                           (__ (_   (_ . _) _) ,__)");
    // kprintln!("    |_._._._._._._._._.|         !                                    `~~`\\ ' . /`~~`");
    // kprintln!("(( / __________________ \\       =o                                         ;   ;");
    // kprintln!("  | OOOOOOOOOOOOOOOOOOO0 |   =                                             /   \\");
    // kprintln!("__________________________________________________________________________/_ __ \\_____________");
    // kprintln!("-------------------------------------------- PANIC --------------------------------------------");
    // kprintln!("          _ ._  _ , _ ._");
    // kprintln!("        (_ ' ( `  )_  .__)");
    // kprintln!("      ( (  (    )   `)  ) _)");
    // kprintln!("     (__ (_   (_ . _) _) ,__)");
    // kprintln!("         `~~`\\ ' . /`~~`");
    // kprintln!("              ;   ;");
    // kprintln!("              /   \\");
    // kprintln!("┌────────────/_ __ \\────────────┐");
    // kprintln!("│             PANIC             │");
    // kprintln!("└───────────────────────────────┘");
    // kprintln!("             \\|/");
    // kprintln!("            .-*-         ");
    // kprintln!("           / /|\\         ");
    // kprintln!("          _L_            ");
    // kprintln!("        ,\"   \".          ");
    // kprintln!("    (\\ /  O O  \\ /)      ");
    // kprintln!("     \\|    _    |/       ");
    // kprintln!("       \\  (_)  /         ");
    // kprintln!("       _/.___,\\_         ");
    // kprintln!("      (_/     \\_)         ");
    // kprintln!("┌───────────────────────┐");
    // kprintln!("│    !!!  PANIC  !!!    │");
    // kprintln!("└───────────────────────┘");
    kprintln!("             . . .");
    kprintln!("              \\|/");
    kprintln!("            `--+--'");
    kprintln!("              /|\\");
    kprintln!("             ' | '");
    kprintln!("               |");
    kprintln!("               |");
    kprintln!("           ,--'#`--.");
    kprintln!("           |#######|");
    kprintln!("        _.-'#######`-._");
    kprintln!("     ,-'###############`-.");
    kprintln!("   ,'#####################`,");
    kprintln!("  /#########################\\");
    kprintln!(" |###########################|");
    kprintln!("|#############################|");
    kprintln!("|#############################|");
    kprintln!("|#############################|");
    kprintln!("|#############################|");
    kprintln!(" |###########################|");
    kprintln!("  \\#########################/");
    kprintln!("   `.#####################,'");
    kprintln!("     `._###############_,'");
    kprintln!("        `--..#####..--'");
    kprintln!("┌─────────────────────────────┐");
    kprintln!("│       !!!  PANIC  !!!       │");
    kprintln!("└─────────────────────────────┘");
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
        None => kprintln!("Panic location cannot be determined"),
    }
    loop {}
}
