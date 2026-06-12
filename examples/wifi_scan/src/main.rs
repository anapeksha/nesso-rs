#![no_std]
#![no_main]

use core::fmt::Write as _;

use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use heapless::String;
use nesso::{
    Nesso,
    bsp::NessoDisplay,
    wifi::{AccessPoint, EspRadioWifiError},
};

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 64 * 1024);
    esp_alloc::heap_allocator!(size: 36 * 1024);
    run(peripherals)
}

fn run(peripherals: esp_hal::peripherals::Peripherals) -> ! {
    let mut delay = Delay::new();
    let mut nesso = match Nesso::new(peripherals) {
        Ok(nesso) => nesso,
        Err(_) => abort(),
    };

    if nesso.display.clear(Rgb565::BLACK).is_err()
        || nesso
            .display
            .print_centered("WiFi Scan", 58, Rgb565::CYAN)
            .is_err()
        || nesso
            .display
            .print_centered("Scanning...", 92, Rgb565::WHITE)
            .is_err()
    {
        abort()
    }

    if !scan_and_render(&mut nesso) {
        abort()
    }

    loop {
        delay.delay_ms(1000);
    }
}

fn scan_and_render(nesso: &mut Nesso) -> bool {
    let mut wifi = match nesso.init_wifi() {
        Ok(wifi) => wifi,
        Err(_) => return false,
    };

    match wifi.scan() {
        Ok(aps) => render_scan_results(&mut nesso.display, &aps),
        Err(error) => {
            let detail = match error {
                EspRadioWifiError::Init => "new failed",
                EspRadioWifiError::ResourcesUnavailable => "resources used",
                EspRadioWifiError::NotStarted => "not started",
                EspRadioWifiError::Configure => "config failed",
                EspRadioWifiError::Scan => "scan failed",
                EspRadioWifiError::Connect => "connect failed",
                EspRadioWifiError::Disconnect => "disconnect failed",
            };
            let _ = nesso.display.clear(Rgb565::BLACK);
            let _ = nesso.display.print_centered("WiFi Scan", 76, Rgb565::CYAN);
            let _ = nesso.display.print_centered(detail, 112, Rgb565::RED);
            false
        }
    }
}

fn render_scan_results(display: &mut NessoDisplay, aps: &[AccessPoint]) -> bool {
    if display.clear(Rgb565::BLACK).is_err()
        || display
            .print_centered("WiFi Scan", 40, Rgb565::CYAN)
            .is_err()
    {
        return false;
    }

    let mut count_line = String::<32>::new();
    let _ = write!(count_line, "APs: {}", aps.len());
    if display
        .print_centered(&count_line, 64, Rgb565::GREEN)
        .is_err()
    {
        return false;
    }

    for (index, ap) in aps.iter().take(3).enumerate() {
        let mut ssid_line = String::<32>::new();
        let mut info_line = String::<32>::new();
        let _ = write!(ssid_line, "{}", ap.ssid.as_str());
        let _ = write!(info_line, "{} dBm ch{}", ap.rssi_dbm, ap.channel);
        let y = 96 + (index as i32 * 42);
        if display
            .print_centered(&ssid_line, y, Rgb565::WHITE)
            .is_err()
            || display
                .print_centered(&info_line, y + 16, Rgb565::YELLOW)
                .is_err()
        {
            return false;
        }
    }

    true
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
