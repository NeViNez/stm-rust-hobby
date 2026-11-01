use embedded_hal::digital::{InputPin, OutputPin};
use stm32f4xx_hal::timer::{Counter, Instance};

#[derive(defmt::Format)]
pub struct Reading {
    pub humidity: u8,
    pub temp_int: u8,
    pub temp_frac: u8,
}

#[derive(defmt::Format)]
pub enum Error {
    NoResponse,
    Checksum,
}

pub struct Dht11<PIN, T> {
    pin: PIN,
    timer: T,
}

pub trait MicrosTimer {
    fn now_mks(&self) -> u32;
}

impl<T: MicrosTimer> MicrosTimer for &T {
    fn now_mks(&self) -> u32 {
        (**self).now_mks()
    }
}

impl<TIM: Instance> MicrosTimer for Counter<TIM, 1_000_000> {
    fn now_mks(&self) -> u32 {
        self.now().ticks()
    }
}

impl<PIN, T> Dht11<PIN, T>
where
    PIN: InputPin + OutputPin,
    T: MicrosTimer,
{
    pub fn new(pin: PIN, timer: T) -> Self {
        Self { pin, timer }
    }
    pub fn read(&mut self) -> Result<Reading, Error> {
        if !dht_init_measure(&mut self.pin, &self.timer) {
            return Err(Error::NoResponse);
        }
        let data = dht_read_data(&mut self.pin, &self.timer).ok_or(Error::Checksum)?;
        Ok(Reading {
            humidity: data[0],
            temp_int: data[2],
            temp_frac: data[3],
        })
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
    let low_mks = wait_while(timer, 200, || dht.is_low().unwrap());
    let high_mks = wait_while(timer, 200, || dht.is_high().unwrap());

    (41..120).contains(&low_mks) && (41..120).contains(&high_mks)
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
    let high_mks = wait_while(timer, 150, || dht.is_high().unwrap());
    high_mks >= 40
}

fn wait_while(timer: &impl MicrosTimer, timeout_mks: u32, mut cond: impl FnMut() -> bool) -> u32 {
    let start = timer.now_mks();
    while cond() && timer.now_mks().wrapping_sub(start) < timeout_mks {}
    timer.now_mks().wrapping_sub(start)
}
