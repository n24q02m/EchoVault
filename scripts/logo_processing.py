import os
import base64
from PIL import Image


def process_logo():
    # File gốc
    input_path = "src-web/public/logo.png"
    # File tạm để xử lý (tránh đọc/ghi cùng lúc nếu muốn)
    # Nhưng vì ta cần đọc ảnh gốc (có nền trắng) nên ta cứ đọc nó.

    # 1. Tạo ảnh trong suốt
    if not os.path.exists(input_path):
        print(f"Lỗi: Không tìm thấy file {input_path}")
        return

    print("Đang đọc và xử lý logo gốc...")
    img = Image.open(input_path)
    img = img.convert("RGBA")

    datas = img.getdata()
    new_data = []
    threshold = 240

    for item in datas:
        if item[0] > threshold and item[1] > threshold and item[2] > threshold:
            new_data.append((255, 255, 255, 0))
        else:
            new_data.append(item)

    img.putdata(new_data)

    # 2. Lưu đè lên file logo.png
    print(f"Đang ghi đè {input_path} (bản trong suốt)...")
    img.save(input_path, "PNG")

    # 3. Lưu file logo_transparent.png (giữ lại nếu user muốn dùng riêng)
    img.save("src-web/public/logo_transparent.png", "PNG")

    # 4. Tạo SVG
    output_svg_path = "src-web/public/logo.svg"
    print(f"Đang tạo {output_svg_path}...")

    # Đọc lại file PNG vừa lưu để encode
    with open(input_path, "rb") as image_file:
        encoded_string = base64.b64encode(image_file.read()).decode("utf-8")

    width, height = img.size
    svg_content = f'''<svg width="{width}" height="{height}" version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
  <image width="{width}" height="{height}" xlink:href="data:image/png;base64,{encoded_string}"/>
</svg>'''

    with open(output_svg_path, "w") as f:
        f.write(svg_content)

    # 5. Cập nhật các icon của Tauri
    tauri_icons_dir = "src-tauri/icons"
    if os.path.exists(tauri_icons_dir):
        print("Đang cập nhật các icon Tauri...")

        # icon.png (thường là 32x32 hoặc lớn hơn tùy config, nhưng cứ lưu đè size gốc hoặc resize nếu cần)
        # Trong tauri.conf.json thường dùng icon.png làm tray icon.
        img.save(os.path.join(tauri_icons_dir, "icon.png"))

        # 32x32.png
        icon_32 = img.resize((32, 32), Image.Resampling.LANCZOS)
        icon_32.save(os.path.join(tauri_icons_dir, "32x32.png"))

        # 128x128.png
        icon_128 = img.resize((128, 128), Image.Resampling.LANCZOS)
        icon_128.save(os.path.join(tauri_icons_dir, "128x128.png"))

        # 128x128@2x.png (256x256)
        icon_256 = img.resize((256, 256), Image.Resampling.LANCZOS)
        icon_256.save(os.path.join(tauri_icons_dir, "128x128@2x.png"))

        # icon.ico
        # ICO có thể chứa nhiều size.
        print("Đang tạo icon.ico...")
        img.save(
            os.path.join(tauri_icons_dir, "icon.ico"),
            format="ICO",
            sizes=[(256, 256), (128, 128), (64, 64), (48, 48), (32, 32), (16, 16)],
        )

    print("Hoàn tất xử lý toàn bộ logo và icon.")


if __name__ == "__main__":
    process_logo()
