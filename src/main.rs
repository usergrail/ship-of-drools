mod error;
mod il2cpp;
mod memory;
mod process;

use std::{
    io::{self, Write},
    thread,
    time::{Duration, Instant},
};

const LB: &str = "\x1b[94m";
const PU: &str = "\x1b[95m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

type Handle = windows::Win32::Foundation::HANDLE;

struct Resources {
    impulser_statics: u64,
    impulser_instance: u64,
    impulser_boat_manager: u64,
    boat_manager_game_state: u64,
    game_state_magazine: u64,
    game_state_cash: u64,
    game_state_plenty_mode: u64,
    game_state_shards: u64,
    network_int_value: u64,
    network_bool_value: u64,
    magazine_plenty_mode: u64,
    magazine_has_capacity: u64,
    magazine_effective_capacity: u64,
    magazine_projectiles_left: u64,
}

impl Resources {
    fn resolve(handle: Handle, module_base: u64) -> Result<Self, String> {
        il2cpp::get_domain(handle, module_base)
            .ok_or_else(|| "il2cpp_domain_get probe failed".to_string())?;
        let image = il2cpp::find_image(handle, module_base, "Assembly-CSharp")
            .or_else(|| il2cpp::find_image(handle, module_base, "Core"))
            .ok_or_else(|| "gameplay image was not found through il2cpp exports".to_string())?;
        let klass = il2cpp::find_class(handle, module_base, image, "", "Impulser")
            .ok_or_else(|| "Impulser class was not found through il2cpp exports".to_string())?;
        let impulser_statics = il2cpp::static_fields(handle, module_base, klass)
            .ok_or_else(|| "Impulser static field data is not initialized".to_string())?;
        let impulser_instance = field(handle, module_base, klass, "<Instance>k__BackingField")?;
        let impulser = read_ptr_at(handle, impulser_statics + impulser_instance)
            .ok_or_else(|| "Impulser instance is not initialized".to_string())?;
        let impulser_boat_manager = field(
            handle,
            module_base,
            object_class(handle, impulser)?,
            "boatManager",
        )?;
        let boat_manager = read_ptr_at(handle, impulser + impulser_boat_manager)
            .ok_or_else(|| "BoatManager is not initialized".to_string())?;
        let boat_manager_game_state = field(
            handle,
            module_base,
            object_class(handle, boat_manager)?,
            "gameState",
        )?;
        let state = read_ptr_at(handle, boat_manager + boat_manager_game_state)
            .ok_or_else(|| "GameState is not initialized".to_string())?;
        let state_class = object_class(handle, state)?;
        let game_state_magazine = field(handle, module_base, state_class, "harpoonMagazine")?;
        let game_state_cash = field(handle, module_base, state_class, "cashCount")?;
        let game_state_plenty_mode = field(handle, module_base, state_class, "plentyMode")?;
        let game_state_shards = field(handle, module_base, state_class, "shardsCount")?;
        let cash = read_ptr_at(handle, state + game_state_cash)
            .ok_or_else(|| "cash NetworkVariable is not initialized".to_string())?;
        let plenty = read_ptr_at(handle, state + game_state_plenty_mode)
            .ok_or_else(|| "plentyMode NetworkVariable is not initialized".to_string())?;
        let magazine = read_ptr_at(handle, state + game_state_magazine)
            .ok_or_else(|| "harpoon Magazine is not initialized".to_string())?;
        let network_int_value = field(
            handle,
            module_base,
            object_class(handle, cash)?,
            "m_InternalValue",
        )?;
        let network_bool_value = field(
            handle,
            module_base,
            object_class(handle, plenty)?,
            "m_InternalValue",
        )?;
        let magazine_class = object_class(handle, magazine)?;
        let magazine_plenty_mode = field(handle, module_base, magazine_class, "plentyMode")?;
        let magazine_has_capacity = field(handle, module_base, magazine_class, "hasCapacity")?;
        let magazine_effective_capacity =
            field(handle, module_base, magazine_class, "effectiveCapacity")?;
        let magazine_projectiles_left =
            field(handle, module_base, magazine_class, "projectilesLeft")?;
        Ok(Self {
            impulser_statics,
            impulser_instance,
            impulser_boat_manager,
            boat_manager_game_state,
            game_state_magazine,
            game_state_cash,
            game_state_plenty_mode,
            game_state_shards,
            network_int_value,
            network_bool_value,
            magazine_plenty_mode,
            magazine_has_capacity,
            magazine_effective_capacity,
            magazine_projectiles_left,
        })
    }

    fn game_state(&self, handle: Handle) -> Option<u64> {
        let impulser = read_ptr_at(handle, self.impulser_statics + self.impulser_instance)?;
        let boat_manager = read_ptr_at(handle, impulser + self.impulser_boat_manager)?;
        read_ptr_at(handle, boat_manager + self.boat_manager_game_state)
    }

    fn squids_addr(&self, handle: Handle) -> Option<u64> {
        let state = self.game_state(handle)?;
        Some(read_ptr_at(handle, state + self.game_state_shards)? + self.network_int_value)
    }

    fn dollars_addr(&self, handle: Handle) -> Option<u64> {
        let state = self.game_state(handle)?;
        Some(read_ptr_at(handle, state + self.game_state_cash)? + self.network_int_value)
    }

    fn harpoons_addr(&self, handle: Handle) -> Option<u64> {
        let state = self.game_state(handle)?;
        let magazine = read_ptr_at(handle, state + self.game_state_magazine)?;
        let uses_capacity =
            memory::try_read_u32(handle, magazine + self.magazine_plenty_mode)? != 0;
        if uses_capacity {
            (memory::try_read_u32(handle, magazine + self.magazine_has_capacity)? != 0)
                .then_some(magazine + self.magazine_effective_capacity)
        } else {
            Some(magazine + self.magazine_projectiles_left)
        }
    }

    fn read_harpoons(&self, handle: Handle) -> Option<u32> {
        let state = self.game_state(handle)?;
        let plenty = read_ptr_at(handle, state + self.game_state_plenty_mode)?;
        if memory::try_read_u32(handle, plenty + self.network_bool_value)? != 0 {
            return Some(i32::MAX as u32);
        }
        memory::try_read_u32(handle, self.harpoons_addr(handle)?)
    }
}

fn wait_for_resources(handle: Handle, module_base: u64) -> Result<Resources, String> {
    const ATTACH_TIMEOUT: Duration = Duration::from_secs(60);
    const RETRY_DELAY: Duration = Duration::from_millis(250);

    let started = Instant::now();
    let mut last_error = String::new();
    loop {
        match Resources::resolve(handle, module_base) {
            Ok(resources) => {
                if !last_error.is_empty() {
                    println!("\r[+] IL2CPP runtime ready                         ");
                }
                return Ok(resources);
            }
            Err(error) => {
                if error != last_error {
                    eprintln!("[*] waiting for IL2CPP runtime: {error}");
                    last_error = error;
                }
            }
        }
        if started.elapsed() >= ATTACH_TIMEOUT {
            return Err(format!(
                "timed out waiting for the IL2CPP runtime after 60 seconds ({last_error})"
            ));
        }
        thread::sleep(RETRY_DELAY);
    }
}

fn read_ptr_at(handle: Handle, address: u64) -> Option<u64> {
    memory::try_read_u64(handle, address).filter(|&p| memory::is_valid_user_ptr(p))
}

fn object_class(handle: Handle, object: u64) -> Result<u64, String> {
    read_ptr_at(handle, object).ok_or_else(|| "managed object class is unavailable".to_string())
}

fn field(handle: Handle, base: u64, klass: u64, name: &str) -> Result<u64, String> {
    il2cpp::field_offset(handle, base, klass, name)
        .ok_or_else(|| format!("field {name} was not found through IL2CPP metadata"))
}

fn print_banner(handle: Handle, resources: &Resources) {
    let squids = resources
        .squids_addr(handle)
        .and_then(|a| memory::try_read_u32(handle, a));
    let dollars = resources
        .dollars_addr(handle)
        .and_then(|a| memory::try_read_u32(handle, a));
    let harpoons = resources.read_harpoons(handle);

    let sq_str = squids.map_or("?".into(), |v| v.to_string());
    let sd_str = dollars.map_or("?".into(), |v| v.to_string());
    let hp_str = harpoons.map_or("?".into(), |v| v.to_string());

    print!("\x1b[2J\x1b[H");
    println!(" ⌜                                                      ⌝");
    println!(" {LB}   ██████╗ ██████╗  ██████╗  ██████╗ ██╗     ███████╗{RESET}");
    println!(" {LB}   ██╔══██╗██╔══██╗██╔═══██╗██╔═══██╗██║     ██╔════╝{RESET}");
    println!(" {LB}   ██║  ██║██████╔╝██║   ██║██║   ██║██║     ███████╗{RESET}");
    println!(" {LB}   ██║  ██║██╔══██╗██║   ██║██║   ██║██║     ╚════██║{RESET}");
    println!(" {LB}   ██████╔╝██║  ██║╚██████╔╝╚██████╔╝███████╗███████║{RESET}");
    println!(" {LB}   ╚═════╝ ╚═╝  ╚═╝ ╚═════╝  ╚═════╝ ╚══════╝╚══════╝{RESET}");
    println!(" ⌞                                                      ⌟");
    println!(" {DIM}            - A masterpiece made by grail. -{RESET}");
    println!();
    println!(" ⌜                                                      ⌝");
    println!("   {DIM}Current Values:{RESET}");
    println!();
    println!("   Squids:         {PU}{sq_str}{RESET}");
    println!("   Sand Dollars:   {PU}{sd_str}{RESET}");
    println!("   Harpoons:       {PU}{hp_str}{RESET}");
    println!();
    println!("   {DIM}Commands:{RESET}");
    println!();
    println!("   {LB}print{RESET}      | {DIM}Print current values{RESET}");
    println!("   {LB}set{RESET}        | {DIM}Set values{RESET}");
    println!("   {LB}exit{RESET}       | {DIM}Quit{RESET}");
    println!(" ⌞                                                      ⌟");
    println!();
}

fn prompt(msg: &str) -> String {
    print!("{msg}");
    io::stdout().flush().unwrap();
    let mut s = String::new();
    io::stdin().read_line(&mut s).unwrap();
    s.trim().to_string()
}

fn main() {
    print!("\x1b]0;ShipOfFools.exe \u{2502} Attaching...\x07");
    let pid = match process::find_pid("ShipOfFools.exe") {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[-] {e}");
            std::process::exit(1);
        }
    };
    let proc = match process::open_process(pid) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[-] {e}");
            std::process::exit(1);
        }
    };
    let module_base = match process::get_module_base(proc.pid, "GameAssembly.dll") {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[-] {e}");
            std::process::exit(1);
        }
    };

    let resources = match wait_for_resources(proc.handle, module_base) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[-] {e}");
            std::process::exit(1);
        }
    };
    print!("\x1b]0;ShipOfFools.exe \u{2502} Attached\x07");

    loop {
        print_banner(proc.handle, &resources);
        let input = prompt(" > ");
        match input.as_str() {
            "exit" => break,
            "print" => {}
            "set" => {
                let sq = prompt(&format!("  Set Squids to {DIM}(blank to skip){RESET}: "));
                if let Ok(v) = sq.parse::<u32>() {
                    write_resource(proc.handle, resources.squids_addr(proc.handle), v);
                }
                let sd = prompt(&format!(
                    "  Set Sand Dollars to {DIM}(blank to skip){RESET}: "
                ));
                if let Ok(v) = sd.parse::<u32>() {
                    write_resource(proc.handle, resources.dollars_addr(proc.handle), v);
                }
                let hp = prompt(&format!("  Set Harpoons to {DIM}(blank to skip){RESET}: "));
                if let Ok(v) = hp.parse::<u32>() {
                    write_resource(proc.handle, resources.harpoons_addr(proc.handle), v);
                }
            }
            _ => {}
        }
    }
}

fn write_resource(handle: Handle, address: Option<u64>, value: u32) {
    let Some(addr) = address else {
        eprintln!("[-] resource address is not currently available");
        return;
    };
    let before = memory::try_read_u32(handle, addr).unwrap_or(0);
    match memory::write_u32(handle, addr, value) {
        Ok(()) => println!("{DIM}  0x{addr:016X}  {before} → {value}{RESET}"),
        Err(e) => eprintln!("[-] write failed: {e}"),
    }
}
