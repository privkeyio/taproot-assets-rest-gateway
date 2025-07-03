use super::addresses;
use super::assets;
use super::burn;
use super::channels;
use super::events;
use super::health;
use super::info;
use super::mailbox;
use super::proofs;
use super::rfq;
use super::send;
use super::stop;
use super::universe;
use super::wallet;
use actix_web::web;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1/taproot-assets")
            .configure(addresses::configure)
            .configure(assets::configure)
            .configure(burn::configure)
            .configure(channels::configure)
            .configure(events::configure)
            .configure(info::configure)
            .configure(mailbox::configure)
            .configure(proofs::configure)
            .configure(rfq::configure)
            .configure(send::configure)
            .configure(stop::configure)
            .configure(universe::configure)
            .configure(wallet::configure),
    )
    .configure(health::configure);
}
