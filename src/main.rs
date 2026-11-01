#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use cortex_m_rt::entry;
use embedded_hal::digital::{InputPin, OutputPin};
use stm32f4xx_hal::{pac, prelude::*, rcc::Config, timer::Counter};

#[entry]
fn main() -> ! {
    // Получаем доступ к периферии STM32
    let dp = pac::Peripherals::take().unwrap();

    // Настраиваем тактирование: HSI, 16 МГц
    let mut rcc = dp.RCC.freeze(Config::hsi().sysclk(16.MHz()));

    // Получаем доступ к порту GPIOA
    let gpioa = dp.GPIOA.split(&mut rcc);
    // PA0 — пин к DATA DHT11
    let mut dht = gpioa.pa0.into_open_drain_output();
    dht.set_high();

    let mut timer = dp.TIM5.counter_us(&mut rcc);
    timer.start(u32::MAX.micros()).unwrap();

    // Основной бесконечный цикл
    loop {
        let cycle_start = timer.now_us();

        if dht_init_measure(&mut dht, &timer) {
            match dht_read_data(&mut dht, &timer) {
                Some(d) => defmt::info!("hum = {}%, temp = {}.{} C", d[0], d[2], d[3]),
                None => defmt::warn!("DHT11: Ошибка контрольной суммы"),
            }
        } else {
            defmt::warn!("DHT11: Ошибка связи с датчиком");
        }

        // Даем датчику отдохнуть перед следующим циклом измерения
        while timer.now_us().wrapping_sub(cycle_start) < 100_000_000 {}
    }
}

fn dht_init_measure(dht: &mut (impl InputPin + OutputPin), timer: &impl MicrosTimer) -> bool {
    // Стартовый импульс: 20 мс низкого уровня
    dht.set_low().unwrap();
    wait_while(timer, 20_000, || true);
    dht.set_high().unwrap();

    // Подтяжка поднимает линию, затем датчик тянет её к низкому уровню
    wait_while(timer, 100, || dht.is_low().unwrap());
    wait_while(timer, 100, || dht.is_high().unwrap());

    // Ответ датчика: ~80 мкс низкий, ~80 мкс высокий
    let low_us = wait_while(timer, 200, || dht.is_low().unwrap());
    let high_us = wait_while(timer, 200, || dht.is_high().unwrap());

    (41..120).contains(&low_us) && (41..120).contains(&high_us)
}

trait MicrosTimer {
    fn now_us(&self) -> u32;
}

impl<TIM: stm32f4xx_hal::timer::Instance> MicrosTimer for Counter<TIM, 1_000_000> {
    fn now_us(&self) -> u32 {
        self.now().ticks()
    }
}

/// Читает 5 байт и проверяет контрольную сумму. None — сумма не сошлась.
fn dht_read_data(dht: &mut impl InputPin, timer: &impl MicrosTimer) -> Option<[u8; 5]> {
    let mut data = [0u8; 5];
    for b in data.iter_mut() {
        *b = dht_read_byte(dht, timer);
    }
    let sum = data[0]
        .wrapping_add(data[1])
        .wrapping_add(data[2])
        .wrapping_add(data[3]);
    (sum == data[4]).then_some(data)
}

fn dht_read_byte(dht: &mut impl InputPin, timer: &impl MicrosTimer) -> u8 {
    let mut byte = 0u8;
    for _ in 0..8 {
        byte = (byte << 1) | dht_read_bit(dht, timer) as u8;
    }
    byte
}

/// Бит: низкий уровень (~50 мкс), затем высокий: ~26 мкс = 0, ~70 мкс = 1.
fn dht_read_bit(dht: &mut impl InputPin, timer: &impl MicrosTimer) -> bool {
    wait_while(timer, 100, || dht.is_low().unwrap());
    let high_us = wait_while(timer, 150, || dht.is_high().unwrap());
    high_us >= 40
}

fn wait_while(timer: &impl MicrosTimer, timeout_us: u32, mut cond: impl FnMut() -> bool) -> u32 {
    let start = timer.now_us();
    while cond() && timer.now_us().wrapping_sub(start) < timeout_us {}
    timer.now_us().wrapping_sub(start)
}
