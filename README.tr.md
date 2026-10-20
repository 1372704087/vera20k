<img src="docs/images/new-conscirpt-hero-image.png" alt="VERA20K kapak görseli" width="100%">

<p align="center" dir="ltr">
  <a href="README.sv.md" lang="sv">Svenska</a> · <a href="README.zh-CN.md" lang="zh-CN">简体中文</a> · <a href="README.de.md" lang="de">Deutsch</a> · <a href="README.ar.md" lang="ar" dir="rtl">العربية</a> · <a href="README.ru.md" lang="ru">Русский</a> · <a href="README.th.md" lang="th">ไทย</a> · <strong>Türkçe</strong> · <a href="README.md" lang="en">English</a>
  &nbsp;&nbsp;
  <a href="https://github.com/YuriPlanet/vera20k/actions/workflows/macos.yml?query=branch%3Amain" title="Son macOS kütüphane testi çalıştırması (elle başlatılır)"><img src="https://github.com/YuriPlanet/vera20k/actions/workflows/macos.yml/badge.svg?branch=main" alt="macOS kütüphane testleri" height="20" align="middle"></a>
  <a href="https://github.com/YuriPlanet/vera20k/actions/workflows/linux.yml?query=branch%3Amain" title="Son Linux kütüphane testi çalıştırması (elle başlatılır)"><img src="https://github.com/YuriPlanet/vera20k/actions/workflows/linux.yml/badge.svg?branch=main" alt="Linux kütüphane testleri" height="20" align="middle"></a>
  <a href="https://github.com/YuriPlanet/vera20k/actions/workflows/windows.yml?query=branch%3Amain" title="Son Windows kütüphane testi çalıştırması (elle başlatılır)"><img src="https://github.com/YuriPlanet/vera20k/actions/workflows/windows.yml/badge.svg?branch=main" alt="Windows kütüphane testleri" height="20" align="middle"></a>
  <a href="https://discord.gg/kmjRUn5m5F"><img src="https://img.shields.io/badge/Discord-Join-5865F2?style=flat&amp;logo=discord&amp;logoColor=white" alt="Discord'a katılın" height="20" align="middle"></a>
  <img src="https://img.shields.io/badge/QQ_grubu-1097193563-1EBAFC?style=flat&amp;logo=qq&amp;logoColor=white" alt="QQ grubu" height="20" align="middle">
</p>

# VERA20K

VERA20K, özgün oyun motoru `gamemd.exe`'nin topluluk öncülüğünde geliştirilen yeniden
yazımıdır. Orijinal oyun dosyalarını kullandığı için kendi Red Alert 2: Yuri's Revenge
kopyanıza ihtiyacınız var.

VERA20K oyuncular tarafından, oyuncular için yapılıyor ve projenin nereye gideceğine son sözü
oyuncular söylüyor.

<img src="docs/images/vera20k-screenshots.png" alt="VERA20K çatışma ayarları ekranı ve oyun içi görünüm" width="100%">

## Projenin hedefleri

1. Orijinal Red Alert 2: Yuri's Revenge'in oynanışını, görsellerini ve atmosferini korumak.
2. 500×500 haritalarda **30 oyuncuya** ve **20.000 birime** kadar daha büyük savaşları desteklemek.
3. Bilinen RTS özelliklerini ve daha önce hiç görülmemiş bazı özellikleri eklemek.
4. Entegre çok oyunculu istemci

## Mevcut durum

**Aktif geliştirme aşamasında.**

- **Çatışma:** Windows, Linux ve macOS'ta oynanabiliyor. Orijinal ve rastgele oluşturulan
  haritalar, üs kurma, kaynak toplama, çatışma ve kaydetme/yükleme mevcut.
- **Senaryolar:** Görevler başlatılabiliyor; destek henüz tamamlanmış değil.
- **Yapay zekâ:** Orijinal yapay zekânın bazı bölümleri üs kurma ve betiklerle yönetilen saldırıları yürütüyor.
- **Devam eden çalışmalar:** Oynanışın aslına uygunluğu, görseller ve performans.
- **Çok oyunculu mod:** Ağ üzerinden oyun henüz mevcut değil.
- **Ölçek:** 30 oyunculu, 20.000 birimli savaşlar henüz gösterilmiş değil.

## Mimari

Motorun üç ana bölümü vardır:

- **Simülasyon:** Oyun durumunu ve mantığını yönetir.
- **Görüntüleme:** Orijinal oyun kaynaklarını kullanan, orijinal görünümü korumak için
  oluşturulmuş tamamen yeni bir GPU görüntüleme hattı.
- **Uygulama:** Girdi, menüler, ses ve platform entegrasyonunu yönetir, sistemleri birbirine bağlar.

Bu ayrım deterministik oynanışı destekler.

Oklar veri akışını gösterir:

![Uygulama simülasyona komut gönderir ve dünya durumunu alır; görüntülemeye çizim verilerini gönderir.](docs/images/architecture.tr.png)

## Derleme ve çalıştırma

Gerekenler:

- **Oyun:** Red Alert 2: Yuri's Revenge.
- **Rust ve Git:** rustup ile kurulmuş güncel kararlı [Rust](https://rustup.rs/) sürümü ve
  [Git](https://git-scm.com/).
- **Derleme araçları:** Windows'ta, Rust kurulum programının kurmayı önerdiği Visual Studio C++
  derleme araçları; macOS'te `xcode-select --install`; Debian ve Ubuntu'da
  `sudo apt install build-essential libasound2-dev pkg-config`.
- **Grafik:** Vulkan, DirectX 12 ya da Metal destekleyen bir GPU.

VERA20K, Windows, Linux ve macOS üzerinde oynandı.

```sh
git clone https://github.com/YuriPlanet/vera20k.git
cd vera20k
cp config.toml.example config.toml
# Çalıştırmadan önce config.toml dosyasını düzenleyip ra2_dir değerini oyun klasörünüz olarak ayarlayın:
cargo run --release --bin vera20k
```

## Nasıl çalışıyoruz

Kodun büyük bölümünü yapay zekâ kodlama ajanları yazıyor. Ajanlar özgün motoru Ghidra ile
inceliyor, ardından davranışını Rust'a aktarıyor ve [karşılaştırma araçları](tools/native_oracle.md)
ile oyun testleri kullanarak kontrol ediyor. Ajanlar [AGENTS.md](AGENTS.md) dosyasını izler.

## Özelleştirme

İsteğe bağlı özellikler (ör. Ares özellikleri) INI dosyaları üzerinden yapılandırılabilmeli
ve uygun olduğunda istemciden açılıp kapatılabilmelidir.

## Katkıda bulunma

Yardımlarınızı bekliyoruz. Kod yazabilir, motoru yeniden düzenleyebilir, oyunu test edebilir,
fikirlerinizi paylaşabilir veya orijinal oyunla yan yana oynayıp neyin yanlış hissettirdiğini
bize anlatabilirsiniz. Bir PR açın, gerisini biz hallederiz.
Tartışmalar [Discord](https://discord.gg/kmjRUn5m5F) üzerinden ve QQ grubunda (1097193563) yürür.

## Destek

<a href="https://paypal.me/vera20k"><img src="docs/images/paypal-support.svg" alt="VERA20K'yi PayPal üzerinden destekleyin" height="40"></a>

Alınan tüm bağışlar,
[TIBTRE katkı defterine](https://github.com/YuriPlanet/tibtre-ledger) göre katkıda
bulunanlara dağıtılır.

Katkıda bulunanlara ödeme yöntemleri:

<p><img src="https://img.shields.io/badge/PayPal-003087?style=flat&amp;logo=paypal&amp;logoColor=white" alt="PayPal" height="20"> <img src="https://img.shields.io/badge/China_UnionPay-E21836?style=flat&amp;logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9Ii02IC02IDI2OCAxNzIiPjxwYXRoIGQ9Ik01MC4wNDE2MzEsMC4wMDA2Njc1NjMzMDUgTDExMy44MjAyMTEsMC4wMDA2Njc1NjMzMDUgQzEyMi43MjMxMjcsMC4wMDA2Njc1NjMzMDUgMTI4LjI2MDQzOCw3LjI1NzMxMzQ1IDEyNi4xODM1NTgsMTYuMTg5NzY1NyBMOTYuNDkwMDcxNSwxNDMuNjc1NDE2IEM5NC4zOTQ1MzY1LDE1Mi41NzY3NzcgODUuNDgwNzM5LDE1OS44Mzk2NDEgNzYuNTcxNjA1LDE1OS44Mzk2NDEgTDEyLjc5OTI0MzMsMTU5LjgzOTY0MSBDMy45MDg3NjQsMTU5LjgzOTY0MSAtMS42NDA5ODQ0NCwxNTIuNTc2Nzc3IDAuNDM1ODk2MjA4LDE0My42NzU0MTYgTDMwLjE0MTgxODYsMTYuMTg5NzY1NyBDMzIuMjE4Njk5Myw3LjI1NzMxMzQ1IDQxLjEyNzgzMzMsMC4wMDA2Njc1NjMzMDUgNTAuMDQxNjMxLDAuMDAwNjY3NTYzMzA1IiBmaWxsPSIjZmZmIiBzdHJva2U9IiNFMjE4MzYiIHN0cm9rZS13aWR0aD0iMTAiIHN0cm9rZS1saW5lam9pbj0icm91bmQiLz48cGF0aCBkPSJNMTA4LjUxMzI4MywwLjAwMDY2NzU2MzMwNSBMMTgxLjg1NzAxOCwwLjAwMDY2NzU2MzMwNSBDMTkwLjc1ODM3OCwwLjAwMDY2NzU2MzMwNSAxODYuNzQ0NTI3LDcuMjU3MzEzNDUgMTg0LjY1MDU0NiwxNi4xODk3NjU3IEwxNTQuOTYxNzI0LDE0My42NzU0MTYgQzE1Mi44Nzg2MjUsMTUyLjU3Njc3NyAxNTMuNTI4NDI3LDE1OS44Mzk2NDEgMTQ0LjYwODQxMSwxNTkuODM5NjQxIEw3MS4yNjQ2Nzc1LDE1OS44Mzk2NDEgQzYyLjM0NDY2MTUsMTU5LjgzOTY0MSA1Ni44MjQ0NDk1LDE1Mi41NzY3NzcgNTguOTE5OTg1LDE0My42NzU0MTYgTDg4LjYwNzI1MjUsMTYuMTg5NzY1NyBDOTAuNzE1MjI0LDcuMjU3MzEzNDUgOTkuNjA1NzAzNSwwLjAwMDY2NzU2MzMwNSAxMDguNTEzMjgzLDAuMDAwNjY3NTYzMzA1IiBmaWxsPSIjZmZmIiBzdHJva2U9IiNFMjE4MzYiIHN0cm9rZS13aWR0aD0iMTAiIHN0cm9rZS1saW5lam9pbj0icm91bmQiLz48cGF0aCBkPSJNMTc4Ljk0NzgzLDAuMDAwNjY3NTYzMzA1IEwyNDIuNzI2NDA5LDAuMDAwNjY3NTYzMzA1IEMyNTEuNjQxNzYyLDAuMDAwNjY3NTYzMzA1IDI1Ny4xNzkwNzQsNy4yNTczMTM0NSAyNTUuMDg1MDkzLDE2LjE4OTc2NTcgTDIyNS4zOTYyNzEsMTQzLjY3NTQxNiBDMjIzLjMwMDczNiwxNTIuNTc2Nzc3IDIxNC4zODA3MTksMTU5LjgzOTY0MSAyMDUuNDY2OTIyLDE1OS44Mzk2NDEgTDE0MS43MTc4NzksMTU5LjgzOTY0MSBDMTMyLjc5Nzg2MywxNTkuODM5NjQxIDEyNy4yNjUyMTUsMTUyLjU3Njc3NyAxMjkuMzU0NTMyLDE0My42NzU0MTYgTDE1OS4wNDgwMTcsMTYuMTg5NzY1NyBDMTYxLjEyNDg5OSw3LjI1NzMxMzQ1IDE3MC4wMjc4MTQsMC4wMDA2Njc1NjMzMDUgMTc4Ljk0NzgzLDAuMDAwNjY3NTYzMzA1IiBmaWxsPSIjZmZmIiBzdHJva2U9IiNFMjE4MzYiIHN0cm9rZS13aWR0aD0iMTAiIHN0cm9rZS1saW5lam9pbj0icm91bmQiLz48L3N2Zz4%3D" alt="China UnionPay" height="20"> <img src="https://img.shields.io/badge/Alipay-1677FF?style=flat&amp;logo=alipay&amp;logoColor=white" alt="Alipay" height="20"> <img src="https://img.shields.io/badge/Vipps-FF5B24?style=flat&amp;logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjQzLjUgMjEuNSAyNyAyNyI%2BPHBhdGggZD0iTTU3LjMsNDAuNmMzLjcsMCw1LjgtMS44LDcuOC00LjRjMS4xLTEuNCwyLjUtMS43LDMuNS0wLjlzMS4xLDIuMywwLDMuN2MtMi45LDMuOC02LjYsNi4xLTExLjMsNi4xIGMtNS4xLDAtOS42LTIuOC0xMi43LTcuN2MtMC45LTEuMy0wLjctMi43LDAuMy0zLjRzMi41LTAuNCwzLjQsMUM1MC41LDM4LjMsNTMuNSw0MC42LDU3LjMsNDAuNnogTTY0LjIsMjguM2MwLDEuOC0xLjQsMy0zLDMgcy0zLTEuMi0zLTNzMS40LTMsMy0zQzYyLjgsMjUuMyw2NC4yLDI2LjYsNjQuMiwyOC4zeiIgZmlsbD0iI2ZmZiIvPjwvc3ZnPg%3D%3D" alt="Vipps" height="20"> <img src="https://img.shields.io/badge/SEPA-2D4B9A?style=flat&amp;logo=sepa&amp;logoColor=white" alt="SEPA" height="20"> <img src="https://img.shields.io/badge/Bitcoin-F7931A?style=flat&amp;logo=bitcoin&amp;logoColor=white" alt="Bitcoin" height="20"></p>

## Teşekkürler ve yasal bilgiler

OpenRA, XCC Mixer, ModEnc wiki, Project Perfect Mod, Command & Conquer ve Red Alert'in
kaynak kodlarını GPL lisansıyla yayımlayan EA, World-Altering Editor, Final Alert, YRpp,
Ares, Phobos ve daha nicelerine teşekkürler.

[GPLv3](LICENSE-GPL) ile lisanslanmıştır. Bu depo hiçbir oyun dosyası içermez.
Command & Conquer ve Red Alert, Electronic Arts Inc. şirketinin ticari markalarıdır.
Ekran görüntülerindeki oyun grafikleri Electronic Arts'a aittir. VERA20K, Electronic Arts
ile bağlantılı değildir ve Electronic Arts tarafından desteklenmemektedir.

[README.md](README.md) dosyasının çevirisidir; lütfen İngilizce sürümle güncel tutun.
