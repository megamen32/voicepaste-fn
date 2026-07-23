import Foundation
import VoicePasteLib

func argumentValue(_ name: String, in arguments: [String]) -> String? {
    guard let index = arguments.firstIndex(of: name), arguments.indices.contains(index + 1) else {
        return nil
    }
    return arguments[index + 1]
}

let arguments = Array(CommandLine.arguments.dropFirst())
guard let endpoint = argumentValue("--endpoint", in: arguments) else {
    fputs("model probe failed: missing required --endpoint\n", stderr)
    exit(EXIT_FAILURE)
}
let apiKey = argumentValue("--api-key", in: arguments) ?? ""

do {
    let client = try ModelListClient(baseURL: endpoint, apiKey: apiKey)
    let models = try client.fetchModels()
    let output = try JSONSerialization.data(withJSONObject: ["models": models])
    print(String(decoding: output, as: UTF8.self))
} catch {
    fputs("model probe failed: \(error.localizedDescription)\n", stderr)
    exit(EXIT_FAILURE)
}
