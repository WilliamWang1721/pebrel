// Native capture/input adapted from qa-macos.py at abfc1a39b681225c0ec6a6faf73098b19f4e42c8.
import Foundation
import Vision
import ImageIO
import CoreGraphics

func frame(_ path: String) -> CGImage {
    let source = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, nil)!
    return CGImageSourceCreateImageAtIndex(source, 0, nil)!
}
let args = CommandLine.arguments
let display = CGDisplayBounds(CGMainDisplayID())
if args[1] == "--move" || args[1] == "--scroll" || args[1] == "--click" {
    guard CGPreflightPostEventAccess() else { fatalError("native event posting permission unavailable") }
    let point = CGPoint(x: Double(args[2])!, y: Double(args[3])!)
    CGWarpMouseCursorPosition(point)
    if args[1] == "--click" {
        let source = CGEventSource(stateID: .combinedSessionState)
        let moved = CGEvent(mouseEventSource: source, mouseType: .mouseMoved,
                            mouseCursorPosition: point, mouseButton: .left)!
        moved.post(tap: .cgSessionEventTap)
        Thread.sleep(forTimeInterval: 0.2)
        for type in [CGEventType.leftMouseDown, CGEventType.leftMouseUp] {
            let click = CGEvent(mouseEventSource: source, mouseType: type,
                                mouseCursorPosition: point, mouseButton: .left)!
            click.setIntegerValueField(.mouseEventClickState, value: 1)
            click.post(tap: .cgSessionEventTap)
            Thread.sleep(forTimeInterval: 0.12)
        }
        exit(0)
    }
    let event: CGEvent
    if args[1] == "--scroll" {
        event = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1,
                        wheel1: Int32(args[4])!, wheel2: 0, wheel3: 0)!
    } else {
        event = CGEvent(mouseEventSource: nil, mouseType: .mouseMoved,
                        mouseCursorPosition: point, mouseButton: .left)!
    }
    event.post(tap: .cgSessionEventTap)
    exit(0)
}
if args[1] == "--diff" {
    let first = frame(args[2]), second = frame(args[3])
    let scale = Double(first.width) / Double(display.width)
    let area = CGRect(x: Double(args[4])! * scale, y: Double(args[5])! * scale,
                      width: Double(args[6])! * scale, height: Double(args[7])! * scale).integral
    let a = first.cropping(to: area)!, b = second.cropping(to: area)!
    func rgba(_ image: CGImage) -> [UInt8] {
        var bytes = [UInt8](repeating: 0, count: image.width * image.height * 4)
        bytes.withUnsafeMutableBytes { data in
            let context = CGContext(data: data.baseAddress, width: image.width, height: image.height,
                                    bitsPerComponent: 8, bytesPerRow: image.width * 4,
                                    space: CGColorSpaceCreateDeviceRGB(),
                                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
            context.draw(image, in: CGRect(x: 0, y: 0, width: CGFloat(image.width), height: CGFloat(image.height)))
        }
        return bytes
    }
    let left = rgba(a), right = rgba(b)
    var count = 0, minX = a.width, minY = a.height, maxX = 0, maxY = 0
    for y in 0..<a.height {
        for x in 0..<a.width {
            let i = (y * a.width + x) * 4
            if (0..<3).contains(where: { abs(Int(left[i + $0]) - Int(right[i + $0])) > 12 }) {
                count += 1
                minX = min(minX, x); minY = min(minY, y)
                maxX = max(maxX, x); maxY = max(maxY, y)
            }
        }
    }
    let changedBounds: [Double] = count == 0 ? [] : [
        Double(area.minX) / scale + Double(minX) / scale,
        Double(area.minY) / scale + Double(minY) / scale,
        Double(maxX - minX + 1) / scale, Double(maxY - minY + 1) / scale,
    ]
    let result: [String: Any] = ["changed_pixels": count, "bounds": changedBounds]
    print(String(data: try JSONSerialization.data(withJSONObject: result), encoding: .utf8)!)
    exit(0)
}
let image = frame(args[1])
let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.recognitionLanguages = ["en-US"]
request.usesLanguageCorrection = false
try VNImageRequestHandler(cgImage: image, options: [:]).perform([request])
let words: [[String: Any]] = (request.results ?? []).compactMap { item in
    guard let text = item.topCandidates(1).first else { return nil }
    let box = item.boundingBox
    return ["text": text.string, "x": box.midX * display.width,
            "y": (1 - box.midY) * display.height,
            "width": box.width * display.width, "height": box.height * display.height]
}
let result: [String: Any] = ["words": words, "image_width": image.width,
    "image_height": image.height, "display_width": display.width, "display_height": display.height]
print(String(data: try JSONSerialization.data(withJSONObject: result), encoding: .utf8)!)
