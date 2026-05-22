//! Reference resolution: imports, name-matching, framework patterns.
//!
//! TODO: import-resolver (tsconfig path aliases, cargo workspace members),
//! name-matcher, frameworks/{express,laravel,rails,fastapi,django,flask,
//! spring,gin,axum,aspnet,vapor,react-router,sveltekit,vue-nuxt,cargo,nestjs,
//! drupal}. Emits route nodes + references edges.

pub mod frameworks;
pub mod imports;
pub mod name_match;
