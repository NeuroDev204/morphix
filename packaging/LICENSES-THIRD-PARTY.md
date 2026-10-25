# Giấy phép các thành phần đi kèm (gói `.deb` Linux)

Tệp này liệt kê mọi thứ mà gói `morphix_<phiên bản>_amd64.deb` mang theo, ngoài mã của Morphix.
Đây là bản ghi kỹ thuật để rà soát, **không phải tư vấn pháp lý**. Trước khi phát hành công khai
cần một đợt rà soát giấy phép đầy đủ (mục 5).

## 0. Morphix

Mã của Morphix phát hành theo **GPL-3.0-or-later** (`LICENSE`). GPL-3.0-or-later tương thích với các thành phần GPL-2.0-or-later bên dưới (ffmpeg, libx264).

## 0b. Bản cài bằng lệnh curl (`morphix-x86_64-linux.tar.gz`)

Khác gói `.deb`, tarball mang ffmpeg tĩnh của BtbN/FFmpeg-Builds (`linux64-gpl`, FFmpeg n8.1.3, có libx264, cấu hình `--enable-gpl --enable-version3`, tức GPL-3.0) tại `lib/morphix/ffmpeg`; giấy phép ở `share/doc/morphix/ffmpeg/LICENSE.txt`, nguồn và lời đề nghị mã nguồn ở `share/doc/morphix/ffmpeg-source-offer.txt` (bản gốc: `packaging/ffmpeg-static.txt`). Giấy phép của từng thư viện mà bản dựng tĩnh gộp vào (dav1d, libvpx, aom, x264...) ở `share/doc/morphix/ffmpeg/third-party/<thư viện>/`, lấy đúng commit BtbN đã dựng bằng `scripts/collect-ffmpeg-licenses.py` (bản lưu: `packaging/ffmpeg-static-licenses/`). Giấy phép crate Rust và gói npm nằm trong binary ở `share/doc/morphix/bundled/` (cả gói `.deb`: `/usr/share/doc/morphix/bundled/`), sinh lúc dựng bằng `scripts/collect-bundled-licenses.py`. PDFium giống gói `.deb`.

## 1. Thành phần chính

| Thành phần | Phiên bản | Giấy phép | Đường dẫn trong gói | Ghi chú |
|---|---|---|---|---|
| ffmpeg (bản GPL, có libx264, libx265) | 7:6.1.1-3ubuntu5 (upstream 6.1.1) | GPL-2.0-or-later (do `--enable-gpl`) | `/usr/lib/morphix/ffmpeg` | Chạy như tiến trình riêng. Cấu hình biên dịch và cách lấy mã nguồn: `/usr/share/doc/morphix/ffmpeg-source-offer.txt` |
| libx264 | 2:0.164.3108+git31e19f9-1 | GPL-2.0-or-later | `/usr/lib/morphix/lib/libx264.so.164` | Encoder H.264. Mã nguồn: gói `x264` của Ubuntu |
| libx265 | 3.5-2build1 | GPL-2.0-or-later | `/usr/lib/morphix/lib/libx265.so.199` | Encoder HEVC. Mã nguồn: gói `x265` của Ubuntu |
| PDFium (`libpdfium.so`) | chromium/7881, VERSION 151.0.7881.0 | BSD kiểu 3 điều khoản (The PDFium Authors); bản dựng pdfium-binaries là MIT (Benoit Blanchon) | `/usr/lib/morphix/pdfium/libpdfium.so` | Nạp bằng `dlopen` từ morphix. Văn bản: `/usr/share/doc/morphix/pdfium/` (LICENSE và `licenses/`, gồm FreeType, ICU, libpng, libjpeg-turbo, zlib, OpenJPEG, LCMS, v.v.) |
| pdfium-render 0.9.4 (crate Rust, liên kết tĩnh vào morphix) | 0.9.4 | MIT OR Apache-2.0 | trong binary `morphix` | Kiểm trên crates.io |
| Các thư viện khác của ffmpeg | xem Phụ lục A | phần lớn LGPL-2.1+, BSD, MIT, ISC; một số GPL-2+ hoặc GPL-3+ | `/usr/lib/morphix/lib/` | Mỗi gói có file copyright tại `/usr/share/doc/morphix/third-party/<gói>/copyright` |
| x11rb và x11rb-protocol (crate Rust) | 0.13.2 | MIT OR Apache-2.0 | trong binary `morphix-gui` | XTest qua giao thức X11, không cần thêm `libXtst` |
| gethostname (crate Rust, phụ thuộc x11rb) | 1.1.0 | Apache-2.0 | trong binary `morphix-gui` | Đối chiếu Cargo.toml của crate khi dựng gói |
| Morphix (`morphix`, `morphix-gui`) | 0.1.0 | Chưa chọn | `/usr/bin/` | Xem `/usr/share/doc/morphix/copyright` |

## 2. Thư viện hệ thống không đóng gói (lấy qua Depends)

Các thư viện này có sẵn trên Ubuntu cùng WebKitGTK 4.1 và AppIndicator, nên gói không mang theo:
`libc6`, `libgcc-s1`, `libstdc++6`, `libglib2.0-0t64`, `libx11-6`, `libcairo2`, `libpango-1.0-0`,
`libfontconfig1`, `libfreetype6`, `libharfbuzz0b`, `libxml2`, `libicu74`, `libgcrypt20`, và các
gói khác trong `crates/morphix-gui/packaging/deb/rely-packages.txt`.

Gói khai báo:
- `libwebkit2gtk-4.1-0`, `libayatana-appindicator3-1` (cho morphix-gui).
- `libc6 (>= 2.39)`: các binary đóng gói và ffmpeg bản Noble cần symbol `GLIBC_2.38`/`GLIBC_2.39`.
  Ubuntu 22.04 (glibc 2.35) **không cài được** gói này. Xem mục 4.

## 3. Mã nguồn tương ứng

- ffmpeg, x264, x265 và các thư viện GPL khác trong Phụ lục A: lấy từ kho nguồn Ubuntu
  (`apt-get source ffmpeg=7:6.1.1-3ubuntu5` và các gói nguồn tương ứng).
- Lời đề nghị bằng văn bản: `/usr/share/doc/morphix/ffmpeg-source-offer.txt`.
- PDFium: tag `chromium/7881` của bblanchon/pdfium-binaries, tệp `pdfium-linux-x64.tgz`,
  sha256 `1470e21b8b4a3b4ad7f85684e2da11d94f3b69a86d81dee11b9b6709d927ac1d`;
  `libpdfium.so` sha256 `f728930966f503652b92acc89b9374a2eeca00ce42e26dccd3e4b5c5161b2d64`.

## 4. Ràng buộc đã biết (Ruling)

- **Ruling: glibc tối thiểu 2.39.** Build trên Ubuntu 24.04. ffmpeg và các thư viện Noble
  dùng `GLIBC_2.38`, binary morphix dùng `GLIBC_2.39`. Gói chạy trên Ubuntu 24.04 trở lên.
  Ubuntu 22.04 không đáp ứng. Để hỗ trợ 22.04 cần dựng lại ffmpeg và các thư viện trên 22.04,
  đây là việc riêng, chưa làm.
- **Ruling: không đóng gói WebKitGTK và các gói GTK.** Chúng nằm trong Depends.
- **Ruling: bản ffmpeg là bản Ubuntu, không tự biên dịch.** Đường dẫn nguồn ffmpeg là
  `/usr/bin/ffmpeg` (gói dpkg `ffmpeg 7:6.1.1-3ubuntu5`), script kiểm đúng phiên bản này.

## 5. Việc còn lại trước khi phát hành công khai

1. Rà soát giấy phép đầy đủ của mọi thư viện trong Phụ lục A và các crate Rust liên kết tĩnh
   (`Cargo.lock`). Đây là việc của người phát hành, không phải kết luận của tài liệu này.
2. Đưa vào gói (hoặc cung cấp kèm) mã nguồn GPL thực sự, không chỉ lời đề nghị.
3. Chọn giấy phép cho Morphix (hiện "chưa chọn").


## Phụ lục A: thư viện chia sẻ đóng gói (soname, gói nguồn, giấy phép theo copyright của gói)

Giấy phép ở đây là các thẻ `License:` trong file copyright của gói, lấy tự động lúc đóng gói. Đây là điểm xuất phát cho rà soát, không phải kết luận pháp lý. Văn bản đầy đủ nằm ở `/usr/share/doc/morphix/third-party/<gói>/copyright`.

| Soname | Gói Ubuntu | Thẻ giấy phép |
|---|---|---|
| `libaom.so.3` | libaom3 | BSD-2-clause BSD-2-Clause BSD-3-clause Expat ISC public-domain-md5 |
| `libapparmor.so.1` | libapparmor1 | BSD-3-clause GPL-2 GPL-2+ LGPL-2.1+ |
| `libasound.so.2` | libasound2t64 | LPGL-2.1+ |
| `libass.so.9` | libass9 | GPL-2+ ISC NTP Unlicense |
| `libasyncns.so.0` | libasyncns0 | LGPL-2.1+ |
| `libavc1394.so.0` | libavc1394-0 | GPL-2+ LGPL-2.1+ |
| `libavcodec.so.60` | libavcodec60 | BSD-1-clause BSD-2-clause BSD-3-clause BSL Expat GPL-2+ |
| `libavdevice.so.60` | libavdevice60 | BSD-1-clause BSD-2-clause BSD-3-clause BSL Expat GPL-2+ |
| `libavfilter.so.9` | libavfilter9 | BSD-1-clause BSD-2-clause BSD-3-clause BSL Expat GPL-2+ |
| `libavformat.so.60` | libavformat60 | BSD-1-clause BSD-2-clause BSD-3-clause BSL Expat GPL-2+ |
| `libavutil.so.58` | libavutil58 | BSD-1-clause BSD-2-clause BSD-3-clause BSL Expat GPL-2+ |
| `libblas.so.3` | libblas3 | BSD-3-clause BSD-3-clause-intel |
| `libblkid.so.1` | libblkid1 | BSD-3-clause BSD-4-clause BSLA GPL-2 GPL-2+ GPL-3+ |
| `libbluray.so.2` | libbluray2 | BSD-3-clause custom GPL-2+ LGPL-2.1+ MPL-1.0 |
| `libbrotlicommon.so.1` | libbrotli1 | MIT |
| `libbrotlidec.so.1` | libbrotli1 | MIT |
| `libbrotlienc.so.1` | libbrotli1 | MIT |
| `libbs2b.so.0` | libbs2b0 | Expat FSF-unlimited GPL-2+ GPL-3+ MIT+FSF-public |
| `libbsd.so.0` | libbsd0 | Beerware BSD-2-clause BSD-2-clause-author BSD-2-clause-NetBSD BSD-2-clause-verbatim BSD-3-clause |
| `libbz2.so.1.0` | libbz2-1.0 | BSD-variant GPL-2 |
| `libcaca.so.0` | libcaca0 | GPL GPL-2+ ISC LGPL-2+ WTFPL-2 |
| `libcap.so.2` | libcap2 | BSD-3-clause GPL-2 GPL-2+ |
| `libcdio_cdda.so.2` | libcdio-cdda2t64 | (xem copyright) |
| `libcdio_paranoia.so.2` | libcdio-paranoia2t64 | (xem copyright) |
| `libcdio.so.19` | libcdio19t64 | GFDL-1.2 GPL-2+ GPL-3 |
| `libchromaprint.so.1` | libchromaprint1 | BSD-3-clause Expat LGPL-2.1+ |
| `libcjson.so.1` | libcjson1 | Apache-2.0 MIT |
| `libcodec2.so.1.2` | libcodec2-1.2 | JMVBSD KISSFFTBSD LGPL-2.1 |
| `libcom_err.so.2` | libcom-err2 | Apache-2 BSD-3-Clause GPL GPL-2 GPL-2+ ISC |
| `libcrypto.so.3` | libssl3t64 | Apache-2.0 Artistic GPL-1+ |
| `libdatrie.so.1` | libdatrie1 | GPL-2+ LGPL-2.1+ |
| `libdav1d.so.7` | libdav1d7 | BSD-2-clause ISC public-domain |
| `libdb-5.3.so` | libdb5.3t64 | Artistic BSD-3-clause BSD-3-clause-fjord GPL GPL-3 MIT-old |
| `libdbus-1.so.3` | libdbus-1-3 | AFL-2.1 autoconf-archive-permissive BSD-3-clause BSD-3-clause-generic Expat FSF-unlimited-permission |
| `libdc1394.so.25` | libdc1394-25 | (xem copyright) |
| `libdecor-0.so.0` | libdecor-0-0 | Expat GPL-3.0+ |
| `libffi.so.8` | libffi8 | Expat GPL GPL-2+ GPL-3+ MPL-1.1 public-domain |
| `libfftw3.so.3` | libfftw3-double3 | GPL-2+ |
| `libFLAC.so.12` | libflac12t64 | BSD-3-clause GFDL-1.1+ GPL-2+ ISC LGPL-2+ LGPL-2.1+ |
| `libflite_cmulex.so.1` | libflite1 | (xem copyright) |
| `libflite_cmu_us_awb.so.1` | libflite1 | (xem copyright) |
| `libflite_cmu_us_kal16.so.1` | libflite1 | (xem copyright) |
| `libflite_cmu_us_kal.so.1` | libflite1 | (xem copyright) |
| `libflite_cmu_us_rms.so.1` | libflite1 | (xem copyright) |
| `libflite_cmu_us_slt.so.1` | libflite1 | (xem copyright) |
| `libflite.so.1` | libflite1 | (xem copyright) |
| `libflite_usenglish.so.1` | libflite1 | (xem copyright) |
| `libfribidi.so.0` | libfribidi0 | LGPL-2.1+ |
| `libgfortran.so.5` | libgfortran5 | (xem copyright) |
| `libGLdispatch.so.0` | libglvnd0 | Apache-2.0 BSD-1-clause GPL GPL-3+ MIT public-domain |
| `libGL.so.1` | libgl1 | Apache-2.0 BSD-1-clause GPL GPL-3+ MIT public-domain |
| `libGLX.so.0` | libglx0 | Apache-2.0 BSD-1-clause GPL GPL-3+ MIT public-domain |
| `libgme.so.0` | libgme0 | LGPL-2.1+ MIT |
| `libgmp.so.10` | libgmp10 | GPL-2+ GPL-3+ LGPL-3+ |
| `libgnutls.so.30` | libgnutls30t64 |  Apache-2.0 BSD-3-Clause CC0 Expat GPLv3+ |
| `libgomp.so.1` | libgomp1 | (xem copyright) |
| `libgraphite2.so.3` | libgraphite2-3 | Artistic custom-sil-open-font-license GPL-1+ GPL-2+ LGPL-2.1+ MPL-1.1 |
| `libgsm.so.1` | libgsm1 | TU-Berlin-2.0 |
| `libgssapi_krb5.so.2` | libgssapi-krb5-2 | (xem copyright) |
| `libhogweed.so.6` | libhogweed6t64 | Expat GAP GPL-2 GPL-2+ GPL-3+ LGPL-2+ |
| `libhwy.so.1` | libhwy1t64 | Apache-2.0 |
| `libidn2.so.0` | libidn2-0 | GPL-2+ GPL-3+ LGPL-3+ Unicode |
| `libiec61883.so.0` | libiec61883-0 | (xem copyright) |
| `libjack.so.0` | libjack-jackd2-0 | BSD-2-clause BSD-3-clause Expat GPL-2 GPL-2+ GPL-2~either |
| `libjpeg.so.8` | libjpeg-turbo8 | BSD-3-clause BSD-BY-LC-NE Expat NTP zlib Zlib |
| `libjxl.so.0.7` | libjxl0.7 | BSD-3-clause-Google ISC-License |
| `libjxl_threads.so.0.7` | libjxl0.7 | BSD-3-clause-Google ISC-License |
| `libk5crypto.so.3` | libk5crypto3 | (xem copyright) |
| `libkeyutils.so.1` | libkeyutils1 | GPL-2+ LGPL-2+ |
| `libkrb5.so.3` | libkrb5-3 | (xem copyright) |
| `libkrb5support.so.0` | libkrb5support0 | (xem copyright) |
| `liblapack.so.3` | liblapack3 | BSD-3-clause BSD-3-clause-intel |
| `liblilv-0.so.0` | liblilv-0-0 | 0BSD ISC |
| `liblz4.so.1` | liblz4-1 | BSD-2-clause GPL-2+ |
| `liblzma.so.5` | liblzma5 |  Autoconf config-h GPL-2 GPL-2+ LGPL-2.1+ |
| `libmbedcrypto.so.7` | libmbedcrypto7t64 | Apache-2.0 GPL-2+ |
| `libmd.so.0` | libmd0 | Beerware BSD-2-clause BSD-2-clause-NetBSD BSD-3-clause BSD-3-clause-Aaron-D-Gifford ISC |
| `libmount.so.1` | libmount1 | BSD-3-clause BSD-4-clause BSLA GPL-2 GPL-2+ GPL-3+ |
| `libmp3lame.so.0` | libmp3lame0 | BSD-3-clause GPL-1+ LGPL-2+ LGPL-2.1+ zlib/libpng |
| `libmpg123.so.0` | libmpg123-0t64 | LGPL-2.1 |
| `libmysofa.so.1` | libmysofa1 | BSD-3-clause CC-BY-4.0 CC-BY-SA-3.0 cipic listen-ircam mit-kemar |
| `libncursesw.so.6` | libncursesw6 | BSD-3-clause MIT/X11 X11 |
| `libnettle.so.8` | libnettle8t64 | Expat GAP GPL-2 GPL-2+ GPL-3+ LGPL-2+ |
| `libnorm.so.1` | libnorm1t64 | BSD-2-clause BSD-3-clause BSD-4-clause-UC NRL-2-clause NRL-3-clause |
| `libnuma.so.1` | libnuma1 | (xem copyright) |
| `libogg.so.0` | libogg0 | BSD-3-clause rfc3533-license rfc5334-license |
| `libopenal.so.1` | libopenal1 | Apache-2.0 BSD-3-clause Expat GPL-2+ GPL-3+ LGPL-2+ |
| `libOpenCL.so.1` | ocl-icd-libopencl1 | BSD-2-Clause |
| `libopenjp2.so.7` | libopenjp2-7 | BSD-2 BSD-3 LIBPNG LIBTIFF LIBTIFF-GLARSON LIBTIFF-PIXAR |
| `libopenmpt.so.0` | libopenmpt0t64 | BSD-3-clause BSL-1.0 GNU-All-Permissive-License GNU-All-Permissive-License-FSF GPL-2+ GPL-3+ |
| `libopus.so.0` | libopus0 | BSD-2-clause BSD-3-clause |
| `libp11-kit.so.0` | libp11-kit0 | Apache-2.0 BSD-3-clause customFSFUL customFSFULLRWD FSFAP FSFULLR |
| `libpangoft2-1.0.so.0` | libpangoft2-1.0-0 | Apache-2 Bitstream-Vera Chromium-BSD-style Example ICU LGPL-2+ |
| `libpcre2-8.so.0` | libpcre2-8-0 | BSD-2-clause BSD-3-clause BSD-3-clause-Cambridge public-domain X11 |
| `libpgm-5.3.so.0` | libpgm-5.3-0t64 | BSD-3-clause ISC LGPL-2+ LGPL-2.1 |
| `libpixman-1.so.0` | libpixman-1-0 | (xem copyright) |
| `libplacebo.so.338` | libplacebo338 | CC0 Expat LGPL-2.1+ Zlib |
| `libpocketsphinx.so.3` | libpocketsphinx3 | BSD-2 BSD-3-clause GPL-2+ |
| `libpostproc.so.57` | libpostproc57 | BSD-1-clause BSD-2-clause BSD-3-clause BSL Expat GPL-2+ |
| `libpulsecommon-16.1.so` | libpulse0 | GPL-2+ LGPL-2+ LGPL-2.1+ other |
| `libpulse.so.0` | libpulse0 | GPL-2+ LGPL-2+ LGPL-2.1+ other |
| `librabbitmq.so.4` | librabbitmq4 | BSD-Author Expat |
| `librav1e.so.0` | librav1e0 | BSD-2-clause BSD-2-Clause ISC |
| `libraw1394.so.11` | libraw1394-11 | (xem copyright) |
| `librist.so.4` | librist4 | BSD-1-clause BSD-2-clause ISC public-domain |
| `librom1394.so.0` | libavc1394-0 | GPL-2+ LGPL-2.1+ |
| `librsvg-2.so.2` | librsvg2-2 | 0BSD Apache-2.0 Apache-2.0, Boost-1.0 BSD-2-clause BSD-3-clause |
| `librubberband.so.2` | librubberband2 | BSD-3-clause Expat GPL-2+ other-1 Zlib |
| `libsamplerate.so.0` | libsamplerate0 | BSD-2-clause FSFAP GPL-2+ GPL-3+ |
| `libSDL2-2.0.so.0` | libsdl2-2.0-0 | Apache-2.0 BrownUn_UnCalifornia_ErikCorry BSD-3-clause BSD-3-clause-chromium BSD-3-clause-kitware Expat |
| `libselinux.so.1` | libselinux1 | GPL-2 public-domain |
| `libserd-0.so.0` | libserd-0-0 | 0BSD BSD-3-clause ISC |
| `libsharpyuv.so.0` | libsharpyuv0 | Apache-2.0 BSD-3-Clause |
| `libshine.so.3` | libshine3 | GPL-2+ LGPL-2 |
| `libslang.so.2` | libslang2 | GPL-2+ |
| `libsnappy.so.1` | libsnappy1v5 | BSD-3-Clause CC-BY-3.0 CC-BY-4.0 MIT public-domain |
| `libsndfile.so.1` | libsndfile1 | Apache-2.0 BSD-2-clause BSD-3-clause FSFAP GPL-2+ GPL-3+ |
| `libsndio.so.7` | libsndio7.0 | ISC ISC-packaging |
| `libsodium.so.23` | libsodium23 | BSD-2-clause CC0 GPL-2+ ISC MIT public-domain |
| `libsord-0.so.0` | libsord-0-0 | ISC |
| `libsoxr.so.0` | libsoxr0 | LGPL-2.1+ permissive1 permissive2 Spherepack |
| `libspeex.so.1` | libspeex1 | BSD-3-Clause custom-1 GFDL-1.1-or-later LGPL-2.0-or-later |
| `libsphinxbase.so.3` | libsphinxbase3t64 | BSD-2-clause BSD-2-clause-beyond BSD-3-clause BSD-3-clause-carnegie GPL-2+ lucent |
| `libsratom-0.so.0` | libsratom-0-0 | ISC |
| `libsrt-gnutls.so.1.5` | libsrt1.5-gnutls | BSD-3-clause LGPL-2.1+ MPL-2.0 unlicense Zlib |
| `libssh-gcrypt.so.4` | libssh-gcrypt-4 | BSD-2-clause BSD-3-clause LGPL-2.1 LGPL-2.1+~OpenSSL public-domain |
| `libssl.so.3` | libssl3t64 | Apache-2.0 Artistic GPL-1+ |
| `libSvtAv1Enc.so.1` | libsvtav1enc1d1 | BSD-2-clause BSD-3-clause BSD-3-Clause-Clear Expat ISC LGPL-2.1+ |
| `libswresample.so.4` | libswresample4 | BSD-1-clause BSD-2-clause BSD-3-clause BSL Expat GPL-2+ |
| `libswscale.so.7` | libswscale7 | BSD-1-clause BSD-2-clause BSD-3-clause BSL Expat GPL-2+ |
| `libthai.so.0` | libthai0 | GPL-2+ LGPL-2.1+ |
| `libtheoradec.so.1` | libtheora0 | BSD-3-Clause |
| `libtheoraenc.so.1` | libtheora0 | BSD-3-Clause |
| `libtinfo.so.6` | libtinfo6 | BSD-3-clause MIT/X11 X11 |
| `libtwolame.so.0` | libtwolame0 | LGPL-2+ |
| `libudev.so.1` | libudev1 | CC0-1.0 Expat GPL-2 GPL-2+ LGPL-2.1+ public-domain |
| `libudfread.so.0` | libudfread0 | GPL-2+ LGPL-2.1+ |
| `libunibreak.so.5` | libunibreak5 | unicode-data Zlib |
| `libunistring.so.5` | libunistring5 | FreeSoftware GFDL-NIV-1.2+ GPL-2+ GPL-3+ LGPL-3+ X11 |
| `libusb-1.0.so.0` | libusb-1.0-0 | (xem copyright) |
| `libva-drm.so.2` | libva-drm2 | Expat Expat-advertising GPL-2+ other |
| `libva.so.2` | libva2 | Expat Expat-advertising GPL-2+ other |
| `libva-x11.so.2` | libva-x11-2 | Expat Expat-advertising GPL-2+ other |
| `libvdpau.so.1` | libvdpau1 | Expat other |
| `libvidstab.so.1.1` | libvidstab1.1 | GPL-2.0+ |
| `libvorbisenc.so.2` | libvorbisenc2 | BSD-3-Clause RFC-special |
| `libvorbisfile.so.3` | libvorbisfile3 | BSD-3-Clause RFC-special |
| `libvorbis.so.0` | libvorbis0a | BSD-3-Clause RFC-special |
| `libvpl.so.2` | libvpl2 | Apache-2.0 BSD-3-clause MIT |
| `libvpx.so.9` | libvpx9 | BSD-3-Clause ISC public-domain |
| `libvulkan.so.1` | libvulkan1 | Apache-2.0 MIT |
| `libwayland-cursor.so.0` | libwayland-cursor0 | X11 |
| `libX11-xcb.so.1` | libx11-xcb1 | (xem copyright) |
| `libx264.so.164` | libx264-164 | BSD-3-clause Expat GPL-2+ ISC LGPL-2.1+ public-domain |
| `libx265.so.199` | libx265-199 | Expat GPL-2+ ISC LGPL-2.1+ |
| `libXau.so.6` | libxau6 | (xem copyright) |
| `libxcb-dri3.so.0` | libxcb-dri3-0 | (xem copyright) |
| `libxcb-render.so.0` | libxcb-render0 | (xem copyright) |
| `libxcb-shape.so.0` | libxcb-shape0 | (xem copyright) |
| `libxcb-shm.so.0` | libxcb-shm0 | (xem copyright) |
| `libxcb.so.1` | libxcb1 | (xem copyright) |
| `libxcb-xfixes.so.0` | libxcb-xfixes0 | (xem copyright) |
| `libXcursor.so.1` | libxcursor1 | (xem copyright) |
| `libXdmcp.so.6` | libxdmcp6 | (xem copyright) |
| `libXext.so.6` | libxext6 | (xem copyright) |
| `libXfixes.so.3` | libxfixes3 | (xem copyright) |
| `libXi.so.6` | libxi6 | (xem copyright) |
| `libxkbcommon.so.0` | libxkbcommon0 | (xem copyright) |
| `libXrandr.so.2` | libxrandr2 | (xem copyright) |
| `libXrender.so.1` | libxrender1 | (xem copyright) |
| `libXss.so.1` | libxss1 | (xem copyright) |
| `libxvidcore.so.4` | libxvidcore4 | GPL-2+ LGPL-2+ |
| `libXv.so.1` | libxv1 | (xem copyright) |
| `libzimg.so.2` | libzimg2 | GPL-3+ WTFPL-2 |
| `libzix-0.so.0` | libzix-0-0 | 0BSD ISC |
| `libzmq.so.5` | libzmq5 | LGPL-2.0+ MIT MPL-2.0 |
| `libzstd.so.1` | libzstd1 | BSD-3-clause Expat GPL-2 zlib |
| `libzvbi.so.0` | libzvbi0t64 | BSD-2-Clause BSD-3-Clause GPL-2 GPL-2+ LGPL-2+ LGPL-2.1+ |

## Phụ lục B: bản Ubuntu 22.04 (jammy), `morphix_<phiên bản>_amd64_jammy.deb`

Bản jammy khác bản Noble ở ffmpeg: jammy chỉ có ffmpeg 4.4.x, nên ffmpeg 6.1.1 được build từ tarball gốc của upstream trên jammy, không lấy từ gói Ubuntu. Các thư viện còn lại (x264, x265, zimg, ...) là gói jammy, và bảng Phụ lục A không áp dụng cho bản này. Danh sách gói đi kèm và giấy phép của bản jammy nằm trong `/usr/share/doc/morphix/third-party/<gói>/copyright` của chính gói đó.

- Phiên bản: FFmpeg 6.1.1 (upstream), tarball `https://ffmpeg.org/releases/ffmpeg-6.1.1.tar.xz`, sha256 `8684f4b00f94b85461884c3719382f1261f0d9eb3d59640a1f4ac0873616f968` (tính trên máy build, chưa đối chiếu chữ ký GPG của upstream).
- Mã nguồn tương ứng: đúng tarball trên. Cấu hình biên dịch đầy đủ nằm trong `/usr/share/doc/morphix/ffmpeg-buildconf.txt` của gói.
- Cấu hình: `./configure` với bộ flag của Noble, bỏ đường dẫn riêng của Ubuntu (`--libdir`, `--incdir`) và đổi prefix sang `/opt/ffmpeg-6.1.1`; thêm `--enable-shared --enable-rpath`. Gói jammy không có các thư viện sau nên bị bỏ khỏi cấu hình: libglslang, libjxl, libvpl, libplacebo, librav1e, librist, libsvtav1. Vẫn có `--enable-gpl` và `--enable-libx264`.
- Script build: `scripts/build-ffmpeg-jammy.sh` (chạy trong container ubuntu:22.04).
