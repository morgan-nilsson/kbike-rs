use crate::kbike_rs::ride_state::RideState;

pub trait _DisplayInterfaceTrait {
    fn init_display(&mut self);
    fn update_display(&mut self, ride_state: &RideState);
}

pub struct _DisplayInterface {

}

impl _DisplayInterfaceTrait for _DisplayInterface {
    fn init_display(&mut self) {
        
    }
    fn update_display(&mut self, ride_state: &RideState) {
        defmt::debug!("Display update: {:?}", ride_state);
    }
}