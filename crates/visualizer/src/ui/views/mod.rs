pub mod control_bar;
pub mod network;
pub mod objects;
pub mod tasks;
pub mod robot_inspector;
pub mod shelf_inspector;
pub mod task_inspector;
pub mod bottom;

pub use control_bar::draw as control_bar;
pub use network::draw as network_view;
pub use objects::objects_tab;
pub use tasks::tasks_tab;
pub use robot_inspector::robot_inspector;
pub use shelf_inspector::shelf_inspector;
pub use task_inspector::task_inspector;
pub use bottom::{logs_tab, analytics_tab};
