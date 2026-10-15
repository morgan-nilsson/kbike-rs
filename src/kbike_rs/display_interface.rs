use crate::kbike_rs::ride_state::RideState;

pub trait DisplayInterfaceTrait {
    fn init_display(&mut self);
    fn update_display(&mut self, ride_state: &RideState);
}

pub struct DisplayInterface {

}

impl DisplayInterfaceTrait for DisplayInterface {
    fn init_display(&mut self) {
        
    }
    fn update_display(&mut self, ride_state: &RideState) {
        defmt::debug!("Display update: {:?}", ride_state);
    }
}