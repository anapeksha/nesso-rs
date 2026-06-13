//! Runtime helpers for ESP radio users.
//!
//! The Nesso N1 Wi-Fi and BLE controllers share the ESP radio runtime. Async
//! applications should start this runtime once before spawning tasks that use
//! Wi-Fi or BLE.

use crate::bsp::RadioRuntimeResources;
use esp_hal::{interrupt::software::SoftwareInterruptControl, timer::timg::TimerGroup};

/// Starts the ESP radio runtime from board-owned runtime resources.
///
/// The resources are moved into the runtime and cannot be reused. Most
/// applications should call [`crate::Nesso::start_async_runtime`] instead of
/// using this lower-level helper.
pub fn start_radio_runtime(resources: RadioRuntimeResources) {
    let timg0 = TimerGroup::new(resources.timer_group0);
    let sw_interrupt = SoftwareInterruptControl::new(resources.software_interrupt);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);
}
