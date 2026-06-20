// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "VoicePasteFn",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(name: "voicepaste-fn", targets: ["VoicePasteFn"])
    ],
    targets: [
        .executableTarget(
            name: "VoicePasteFn",
            path: "Sources/VoicePasteFn"
        )
    ]
)
