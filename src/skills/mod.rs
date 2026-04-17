// skills 模块：Agent Skills 功能入口
// 负责子模块声明与公共类型导出，让外部可以通过 `crate::skills::SkillMetadata` 等方式访问

pub mod loader;
pub mod registry;
pub mod skill_type;

// 重新导出常用类型，减少外部调用时的路径层级
pub use skill_type::{SkillEntry, SkillMetadata, SkillRegistry, SkillSourceType};
