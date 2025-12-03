use core::{convert::Infallible, iter, mem, num};
use std::{
    // TODO: choose a better hasher
    collections::hash_map::{Entry, HashMap},
    io,
    thread,
};
use syfala_net::{AudioConfig, network, queue};

pub mod client;
mod interleaver;
pub mod server;
