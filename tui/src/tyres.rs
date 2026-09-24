use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config;
use crate::consts;
use crate::libconfig::{self, Value};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TyreStore {
    pub cars: Vec<TyreCar>,
    pub extra: Vec<(String, Value)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TyreCar {
    pub car: String,
    pub sim: String,
    pub tyre0: f64,
    pub tyre1: f64,
    pub tyre2: f64,
    pub tyre3: f64,
    pub extra: Vec<(String, Value)>,
}

impl Default for TyreCar {
    fn default() -> Self {
        Self {
            car: consts::DEFAULT_CAR.to_string(),
            sim: consts::DEFAULT_SIM.to_string(),
            tyre0: 0.0,
            tyre1: 0.0,
            tyre2: 0.0,
            tyre3: 0.0,
            extra: Vec::new(),
        }
    }
}

pub fn load(path: &Path) -> Result<TyreStore> {
    if !path.exists() {
        return Ok(TyreStore::default());
    }
    parse(&std::fs::read_to_string(path)?)
}

pub fn parse(src: &str) -> Result<TyreStore> {
    let root = libconfig::parse(src).map_err(|err| anyhow!("{err}"))?;
    let Some(group) = root.as_group() else {
        return Err(anyhow!("diameters root must be a group"));
    };
    let mut extra = Vec::new();
    let mut cars = Vec::new();
    for (key, value) in group {
        if key == consts::KEY_CARS {
            let list = value
                .as_list()
                .ok_or_else(|| anyhow!("cars must be a list"))?;
            for item in list {
                cars.push(car_from_value(item)?);
            }
            continue;
        }
        extra.push((key.clone(), value.clone()));
    }
    Ok(TyreStore { cars, extra })
}

fn car_from_value(value: &Value) -> Result<TyreCar> {
    let Some(group) = value.as_group() else {
        return Err(anyhow!("car entry must be a group"));
    };
    let mut car = TyreCar::default();
    let mut extra = Vec::new();
    for (key, item) in group {
        match key.as_str() {
            consts::KEY_CAR => car.car = item.as_str().unwrap_or("").to_string(),
            consts::KEY_SIM => car.sim = item.as_str().unwrap_or("").to_string(),
            consts::KEY_TYRE0 => car.tyre0 = item.as_f64().unwrap_or(0.0),
            consts::KEY_TYRE1 => car.tyre1 = item.as_f64().unwrap_or(0.0),
            consts::KEY_TYRE2 => car.tyre2 = item.as_f64().unwrap_or(0.0),
            consts::KEY_TYRE3 => car.tyre3 = item.as_f64().unwrap_or(0.0),
            _ => extra.push((key.clone(), item.clone())),
        }
    }
    car.extra = extra;
    Ok(car)
}

pub fn to_value(store: &TyreStore) -> Value {
    let mut root = store.extra.clone();
    let cars = store.cars.iter().map(car_to_value).collect();
    libconfig::group_set(&mut root, consts::KEY_CARS, Value::List(cars));
    Value::Group(root)
}

fn car_to_value(car: &TyreCar) -> Value {
    let mut items = car.extra.clone();
    libconfig::group_set(&mut items, consts::KEY_CAR, Value::String(car.car.clone()));
    libconfig::group_set(&mut items, consts::KEY_SIM, Value::String(car.sim.clone()));
    libconfig::group_set(&mut items, consts::KEY_TYRE0, Value::Float(car.tyre0));
    libconfig::group_set(&mut items, consts::KEY_TYRE1, Value::Float(car.tyre1));
    libconfig::group_set(&mut items, consts::KEY_TYRE2, Value::Float(car.tyre2));
    libconfig::group_set(&mut items, consts::KEY_TYRE3, Value::Float(car.tyre3));
    Value::Group(items)
}

pub fn save(path: &Path, store: &TyreStore) -> Result<()> {
    config::atomic_write(path, &libconfig::render(&to_value(store)))
}
