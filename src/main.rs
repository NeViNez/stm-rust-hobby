#![no_std]
#![no_main]

use panic_halt as _;

use cortex_m_rt::entry;
use stm32f4xx_hal::{pac, prelude::*, rcc::Config};

#[entry]
fn main() -> ! {
    // Получаем доступ к периферии STM32
    let dp = pac::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    // Настраиваем тактирование: HSI, 16 МГц
    let mut rcc = dp.RCC.freeze(Config::hsi().sysclk(16.MHz()));

    // Получаем доступ к порту GPIOA
    let gpioa = dp.GPIOA.split(&mut rcc);

    // PA5 — встроенный светодиод LD2
    let mut led = gpioa.pa5.into_push_pull_output();

    // Создаём задержку на основе SysTick
    let mut delay = cp.SYST.delay(&rcc.clocks);

    // Основной бесконечный цикл
    loop {
        led.toggle();
        delay.delay_ms(500);
    }
}
