// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "SensesDesignSystem",
    platforms: [.macOS(.v14)],
    products: [.library(name: "SensesDesignSystem", targets: ["SensesDesignSystem"])],
    targets: [.target(name: "SensesDesignSystem")]
)
