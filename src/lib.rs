#[cfg(any(
  all(feature = "sqlite", feature = "mysql"),
  all(feature = "sqlite", feature = "postgres"),
  all(feature = "sqlite", feature = "mssql"),
  all(feature = "mysql", feature = "postgres"),
  all(feature = "mysql", feature = "mssql"),
  all(feature = "postgres", feature = "mssql")
))]
compile_error!(
  "Only one database driver can be enabled. Set the feature flag for the driver of your choice."
);

#[cfg(not(any(feature = "sqlite", feature = "mysql", feature = "postgres", feature = "mssql")))]
compile_error!(
  "Database driver not defined. Please set the feature flag for the driver of your choice."
);