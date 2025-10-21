#ifndef NOMINMAX
#define NOMINMAX
#endif

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif

#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0601
#endif

#include <windows.h>

#include <chrono>
#include <cmath>
#include <iostream>
#include <type_traits>
#include <vector>

namespace detail {

	template <typename T>
	constexpr T clamp(T value, T min_value, T max_value) {
		return (value < min_value) ? min_value : ((value > max_value) ? max_value : value);
	}

	template <typename T>
	constexpr T abs_val(T value) {
		return (value < 0) ? -value : value;
	}

	template <typename T>
	constexpr T min_val(T a, T b) {
		return (a < b) ? a : b;
	}

	template <typename T>
	constexpr T max_val(T a, T b) {
		return (a > b) ? a : b;
	}

	using PvJoyEnabled = bool(__cdecl*)(void);
	using PGetVJDStatus = int(__cdecl*)(unsigned int);
	using PAcquireVJD = bool(__cdecl*)(unsigned int);
	using PRelinquishVJD = void(__cdecl*)(unsigned int);
	using PSetAxis = bool(__cdecl*)(long, unsigned int, unsigned int);
	using PSetBtn = bool(__cdecl*)(bool, unsigned int, unsigned char);
	using PResetVJD = bool(__cdecl*)(unsigned int);

	enum class VjdStat : int {
		Own = 0,
		Free,
		Busy,
		Missing,
		Unknown
	};

	constexpr unsigned int kHidUsageX = 0x30;
	constexpr unsigned int kHidUsageY = 0x31;
	constexpr unsigned int kHidUsageRz = 0x35;

} // namespace detail

struct Config {
	int thread_interval_ms = 4;

	double mouse_sens = 3.0;
	long mouse_delta_cap = 180;

	double throttle_curve_exp = 2.0;
	double throttle_min_cut_at_full = 0.70;
	int throttle_ramp_ms = 75;
	int throttle_drop_ms = 25;
	bool throttle_use_ema = true;
	double throttle_ema_alpha = 0.30;

	int brake_fast_apply_ms = 10;
	int brake_hold_ms = 1750;
	int brake_release_total_ms = 2500;
	double brake_release_accel_exp = 1.7;
	int brake_fast_release_ms = 65;

	double brake_min_ratio_base = 0.40;
	double brake_min_ratio_max = 0.55;
	double brake_curve_exp = 2.0;
	bool brake_trail_enabled = false;

	double brake_after_release_hold_ratio = 0.06;
	int brake_after_release_hold_ms = 500;
};

class MouseDrive {
public:
	explicit MouseDrive(UINT device_id = 1u) : vjoy_device_(device_id) {
		last_update_ = std::chrono::high_resolution_clock::now();
	}

	~MouseDrive() {
		Cleanup();
	}

	bool Initialize();
	void Run();

private:
	struct VJoyApi {
		HMODULE library = nullptr;
		detail::PvJoyEnabled vJoyEnabled = nullptr;
		detail::PGetVJDStatus GetVJDStatus = nullptr;
		detail::PAcquireVJD AcquireVJD = nullptr;
		detail::PRelinquishVJD RelinquishVJD = nullptr;
		detail::PSetAxis SetAxis = nullptr;
		detail::PSetBtn SetBtn = nullptr;
		detail::PResetVJD ResetVJD = nullptr;

		bool Load();
		void Unload();
		bool Valid() const;
	} vjoy_;

	void Cleanup();
	bool CreateRawInputWindow();
	bool RegisterMouseRawInput();
	void Update();
	void UpdateSteering();
	void UpdateThrottle(double time_scale_ms);
	void UpdateBrake(std::chrono::high_resolution_clock::time_point now, double time_scale_ms);
	void SendToVJoy();
	double RateFromTime(double full_scale, int milliseconds) const;
	double SteeringRatio() const;

	static LRESULT CALLBACK RawWndProc(HWND hWnd, UINT msg, WPARAM wParam, LPARAM lParam);

	static MouseDrive* instance_;

private:
	Config cfg_{};
	UINT vjoy_device_ = 1;

	static constexpr long kAxisMax = 32767;
	static constexpr long kAxisMin = 0;
	static constexpr long kAxisCenter = 16383;
	static constexpr long kSteeringRange = 16383;

	double steering_ = 0.0;
	double steering_filtered_ = 0.0;
	double throttle_ = 0.0;
	double throttle_target_ = 0.0;
	double brake_ = 0.0;

	bool braking_active_ = false;
	std::chrono::high_resolution_clock::time_point brake_start_time_{};
	bool brake_post_hold_started_ = false;
	std::chrono::high_resolution_clock::time_point brake_post_hold_start_time_{};
	double brake_post_hold_start_value_ = 1.0;
	bool brake_release_hold_active_ = false;
	std::chrono::high_resolution_clock::time_point brake_release_hold_start_time_{};

	std::chrono::high_resolution_clock::time_point last_update_{};

	bool left_button_pressed_ = false;
	bool right_button_pressed_ = false;
	bool w_key_pressed_ = false;
	bool s_key_pressed_ = false;
	long mouse_delta_x_ = 0;

	HWND raw_window_ = nullptr;
};

MouseDrive* MouseDrive::instance_ = nullptr;

bool MouseDrive::VJoyApi::Load() {
	Unload();
	library = LoadLibraryW(L"vJoyInterface.dll");
	if (!library) {
		std::wcout << L"vJoyInterface.dll yuklenemedi." << std::endl;
		return false;
	}

	auto load_symbol = [this](auto& target, const char* name) {
		target = reinterpret_cast<std::remove_reference_t<decltype(target)>>(GetProcAddress(library, name));
		return target != nullptr;
		};

	if (!load_symbol(vJoyEnabled, "vJoyEnabled") ||
		!load_symbol(GetVJDStatus, "GetVJDStatus") ||
		!load_symbol(AcquireVJD, "AcquireVJD") ||
		!load_symbol(RelinquishVJD, "RelinquishVJD") ||
		!load_symbol(SetAxis, "SetAxis") ||
		!load_symbol(SetBtn, "SetBtn") ||
		!load_symbol(ResetVJD, "ResetVJD")) {
		std::wcout << L"vJoy fonksiyonlari bulunamadi." << std::endl;
		Unload();
		return false;
	}

	return true;
}

void MouseDrive::VJoyApi::Unload() {
	if (library) {
		FreeLibrary(library);
		library = nullptr;
	}
	vJoyEnabled = nullptr;
	GetVJDStatus = nullptr;
	AcquireVJD = nullptr;
	RelinquishVJD = nullptr;
	SetAxis = nullptr;
	SetBtn = nullptr;
	ResetVJD = nullptr;
}

bool MouseDrive::VJoyApi::Valid() const {
	return library && vJoyEnabled && GetVJDStatus && AcquireVJD && RelinquishVJD && SetAxis && SetBtn && ResetVJD;
}

bool MouseDrive::Initialize() {
	if (!vjoy_.Load() || !vjoy_.Valid()) {
		return false;
	}

	if (!vjoy_.vJoyEnabled()) {
		std::wcout << L"vJoy surucusu etkin degil." << std::endl;
		return false;
	}

	auto status = static_cast<detail::VjdStat>(vjoy_.GetVJDStatus(vjoy_device_));
	if (status != detail::VjdStat::Free) {
		std::wcout << L"vJoy aygiti kullanilabilir degil: " << vjoy_device_ << std::endl;
		return false;
	}

	if (!vjoy_.AcquireVJD(vjoy_device_)) {
		std::wcout << L"vJoy aygiti alinamadi: " << vjoy_device_ << std::endl;
		return false;
	}

	vjoy_.ResetVJD(vjoy_device_);
	instance_ = this;

	if (!CreateRawInputWindow()) {
		std::wcout << L"Raw Input penceresi oluşturulamadi." << std::endl;
		return false;
	}

	if (!RegisterMouseRawInput()) {
		std::wcout << L"Raw Input kaydi basarisiz." << std::endl;
		return false;
	}

	return true;
}

void MouseDrive::Cleanup() {
	if (raw_window_) {
		DestroyWindow(raw_window_);
		raw_window_ = nullptr;
	}

	if (vjoy_.RelinquishVJD && vjoy_device_) {
		vjoy_.RelinquishVJD(vjoy_device_);
	}

	vjoy_.Unload();
	instance_ = nullptr;
}

bool MouseDrive::CreateRawInputWindow() {
	WNDCLASSW wc{};
	wc.lpfnWndProc = RawWndProc;
	wc.hInstance = GetModuleHandleW(nullptr);
	wc.lpszClassName = L"MouseDriveRawWindow";

	if (!RegisterClassW(&wc) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) {
		return false;
	}

	raw_window_ = CreateWindowExW(
		0, wc.lpszClassName, L"MouseDriveRaw",
		WS_POPUP,
		0, 0, 0, 0,
		nullptr, nullptr, wc.hInstance, nullptr);

	return raw_window_ != nullptr;
}

bool MouseDrive::RegisterMouseRawInput() {
	RAWINPUTDEVICE rid{};
	rid.usUsagePage = 0x01;
	rid.usUsage = 0x02;
	rid.dwFlags = RIDEV_INPUTSINK;
	rid.hwndTarget = raw_window_;

	return RegisterRawInputDevices(&rid, 1, sizeof(rid)) == TRUE;
}

void MouseDrive::Run() {
	MSG msg{};
	auto next_update = std::chrono::high_resolution_clock::now();

	std::wcout << L"MouseDrive dönüştürücü başlatildi. Cikmak icin Ctrl+C." << std::endl;
	std::wcout << L"Kontroller:" << std::endl;
	std::wcout << L"- Mouse hareketi: Direksiyon (orta tus ile sifirla)" << std::endl;
	std::wcout << L"- Sol tus: Gaz" << std::endl;
	std::wcout << L"- Sag tus: Fren" << std::endl;
	std::wcout << L"- W/S tuslari: Vites" << std::endl;

	while (true) {
		while (PeekMessageW(&msg, nullptr, 0, 0, PM_REMOVE)) {
			if (msg.message == WM_QUIT) {
				return;
			}
			TranslateMessage(&msg);
			DispatchMessageW(&msg);
		}

		auto now = std::chrono::high_resolution_clock::now();
		if (now >= next_update) {
			Update();
			next_update = now + std::chrono::milliseconds(detail::max_val(cfg_.thread_interval_ms, 1));
		}

		Sleep(1);
	}
}

void MouseDrive::Update() {
	auto now = std::chrono::high_resolution_clock::now();
	double delta_ms = std::chrono::duration<double, std::milli>(now - last_update_).count();
	last_update_ = now;

	double base_interval = detail::max_val(cfg_.thread_interval_ms, 1);
	double time_scale = detail::clamp(delta_ms / base_interval, 0.25, 3.0);

	w_key_pressed_ = (GetAsyncKeyState('W') & 0x8000) != 0;
	s_key_pressed_ = (GetAsyncKeyState('S') & 0x8000) != 0;

	UpdateSteering();
	UpdateThrottle(time_scale);
	UpdateBrake(now, time_scale);
	SendToVJoy();
}

void MouseDrive::UpdateSteering() {
	long dx = mouse_delta_x_;
	mouse_delta_x_ = 0;

	steering_ += dx * cfg_.mouse_sens;
	steering_ = detail::clamp(steering_, static_cast<double>(-kSteeringRange), static_cast<double>(kSteeringRange));
	steering_filtered_ = steering_;
}

double MouseDrive::SteeringRatio() const {
	return detail::clamp(detail::abs_val(steering_filtered_) / static_cast<double>(kSteeringRange), 0.0, 1.0);
}

double MouseDrive::RateFromTime(double full_scale, int milliseconds) const {
	if (milliseconds <= 0) {
		return full_scale;
	}
	return full_scale * (cfg_.thread_interval_ms / static_cast<double>(milliseconds));
}

void MouseDrive::UpdateThrottle(double time_scale_ms) {
	const double start_cut = 0.19;
	const double max_cut_start = 0.80;

	if (left_button_pressed_) {
		double steering_ratio = SteeringRatio();
		double normalized = 0.0;
		if (steering_ratio <= start_cut) {
			normalized = 0.0;
		}
		else if (steering_ratio >= max_cut_start) {
			normalized = 1.0;
		}
		else {
			normalized = (steering_ratio - start_cut) / (max_cut_start - start_cut);
		}

		normalized = detail::clamp(normalized, 0.0, 1.0);
		double shaped = std::pow(normalized, cfg_.throttle_curve_exp);
		throttle_target_ = 1.0 - shaped * (1.0 - cfg_.throttle_min_cut_at_full);
	}
	else {
		throttle_target_ = 0.0;
	}

	double throttle_inc = RateFromTime(1.0, cfg_.throttle_ramp_ms) * time_scale_ms;
	double throttle_dec = RateFromTime(1.0, cfg_.throttle_drop_ms) * time_scale_ms;
	double delta = throttle_target_ - throttle_;

	double step = (delta > 0.0)
		? detail::min_val(delta, throttle_inc)
		: detail::max_val(delta, -throttle_dec);

	double candidate = throttle_ + step;
	if (cfg_.throttle_use_ema) {
		candidate += cfg_.throttle_ema_alpha * (throttle_target_ - candidate);
	}

	throttle_ = detail::clamp(candidate, 0.0, 1.0);
}

void MouseDrive::UpdateBrake(std::chrono::high_resolution_clock::time_point now, double time_scale_ms) {
	auto clear_post_release = [&]() {
		brake_release_hold_active_ = false;
		};

	if (right_button_pressed_) {
		if (!braking_active_) {
			braking_active_ = true;
			brake_start_time_ = now;
			brake_post_hold_started_ = false;
			brake_release_hold_active_ = false;
		}

		double elapsed_ms = std::chrono::duration<double, std::milli>(now - brake_start_time_).count();
		double dyn_min = cfg_.brake_min_ratio_base;
		if (cfg_.brake_trail_enabled) {
			double shaped = std::pow(SteeringRatio(), cfg_.brake_curve_exp);
			dyn_min = cfg_.brake_min_ratio_base + (cfg_.brake_min_ratio_max - cfg_.brake_min_ratio_base) * shaped;
		}

		dyn_min = detail::clamp(dyn_min, 0.0, 1.0);
		double fast_apply = RateFromTime(1.0, cfg_.brake_fast_apply_ms) * time_scale_ms;

		if (elapsed_ms < cfg_.brake_hold_ms) {
			brake_ = detail::clamp(brake_ + fast_apply, dyn_min, 1.0);
		}
		else {
			if (!brake_post_hold_started_) {
				brake_post_hold_started_ = true;
				brake_post_hold_start_time_ = now;
				brake_post_hold_start_value_ = brake_;
			}

			double release_elapsed = std::chrono::duration<double, std::milli>(now - brake_post_hold_start_time_).count();
			double progress = detail::clamp(release_elapsed / detail::max_val(cfg_.brake_release_total_ms, 1), 0.0, 1.0);
			double shaped = std::pow(progress, detail::max_val(cfg_.brake_release_accel_exp, 0.1));
			double target = brake_post_hold_start_value_ - shaped * (brake_post_hold_start_value_ - dyn_min);
			brake_ = detail::clamp(target, dyn_min, 1.0);
		}

		clear_post_release();
	}
	else {
		if (braking_active_) {
			braking_active_ = false;
			brake_post_hold_started_ = false;
			brake_release_hold_active_ = true;
			brake_release_hold_start_time_ = now;
			brake_ = detail::max_val(brake_, cfg_.brake_after_release_hold_ratio);
		}
		else if (brake_release_hold_active_) {
			double elapsed_hold = std::chrono::duration<double, std::milli>(now - brake_release_hold_start_time_).count();
			if (elapsed_hold >= cfg_.brake_after_release_hold_ms) {
				brake_release_hold_active_ = false;
			}
			else {
				brake_ = cfg_.brake_after_release_hold_ratio;
			}
		}

		if (!right_button_pressed_ && !brake_release_hold_active_) {
			double fast_release = RateFromTime(1.0, cfg_.brake_fast_release_ms) * time_scale_ms;
			brake_ = detail::max_val(0.0, brake_ - fast_release);
			brake_post_hold_started_ = false;
		}
	}

	brake_ = detail::clamp(brake_, 0.0, 1.0);
}

void MouseDrive::SendToVJoy() {
	if (!vjoy_.Valid()) {
		return;
	}

	double safe_steering = detail::clamp(steering_filtered_, static_cast<double>(-kSteeringRange), static_cast<double>(kSteeringRange));
	long steering_axis = kAxisCenter + static_cast<long>(std::llround(safe_steering));
	steering_axis = detail::clamp(steering_axis, kAxisMin, kAxisMax);

	long throttle_axis = static_cast<long>(std::llround(detail::clamp(throttle_, 0.0, 1.0) * kAxisMax));
	long brake_axis = static_cast<long>(std::llround(detail::clamp(brake_, 0.0, 1.0) * kAxisMax));

	vjoy_.SetAxis(steering_axis, vjoy_device_, detail::kHidUsageX);
	vjoy_.SetAxis(throttle_axis, vjoy_device_, detail::kHidUsageY);
	vjoy_.SetAxis(brake_axis, vjoy_device_, detail::kHidUsageRz);

	vjoy_.SetBtn(w_key_pressed_, vjoy_device_, 1);
	vjoy_.SetBtn(s_key_pressed_, vjoy_device_, 2);
}

LRESULT CALLBACK MouseDrive::RawWndProc(HWND hWnd, UINT msg, WPARAM wParam, LPARAM lParam) {
	if (!instance_) {
		return DefWindowProcW(hWnd, msg, wParam, lParam);
	}

	switch (msg) {
	case WM_INPUT: {
		UINT size = 0;
		if (GetRawInputData(reinterpret_cast<HRAWINPUT>(lParam), RID_INPUT, nullptr, &size, sizeof(RAWINPUTHEADER)) != 0 || size == 0) {
			break;
		}

		std::vector<BYTE> buffer(size);
		if (GetRawInputData(reinterpret_cast<HRAWINPUT>(lParam), RID_INPUT, buffer.data(), &size, sizeof(RAWINPUTHEADER)) != size) {
			break;
		}

		RAWINPUT* raw = reinterpret_cast<RAWINPUT*>(buffer.data());
		if (raw->header.dwType != RIM_TYPEMOUSE) {
			break;
		}

		const RAWMOUSE& mouse = raw->data.mouse;
		if (!(mouse.usFlags & MOUSE_MOVE_ABSOLUTE)) {
			LONG dx = mouse.lLastX;
			dx = detail::clamp(dx, -instance_->cfg_.mouse_delta_cap, instance_->cfg_.mouse_delta_cap);
			instance_->mouse_delta_x_ += dx;
		}

		USHORT flags = mouse.usButtonFlags;
		if (flags & RI_MOUSE_LEFT_BUTTON_DOWN) instance_->left_button_pressed_ = true;
		if (flags & RI_MOUSE_LEFT_BUTTON_UP) instance_->left_button_pressed_ = false;
		if (flags & RI_MOUSE_RIGHT_BUTTON_DOWN) instance_->right_button_pressed_ = true;
		if (flags & RI_MOUSE_RIGHT_BUTTON_UP) instance_->right_button_pressed_ = false;

		if (flags & RI_MOUSE_MIDDLE_BUTTON_DOWN) {
			instance_->steering_ = 0.0;
			instance_->steering_filtered_ = 0.0;
		}
		break;
	}
	default:
		break;
	}

	return DefWindowProcW(hWnd, msg, wParam, lParam);
}

BOOL WINAPI ConsoleHandler(DWORD signal) {
	if (signal == CTRL_C_EVENT) {
		PostQuitMessage(0);
		return TRUE;
	}
	return FALSE;
}

int main() {
	MouseDrive app;
	if (!app.Initialize()) {
		std::wcout << L"Uygulama baslatilamadi." << std::endl;
		std::wcin.get();
		return -1;
	}

	SetConsoleCtrlHandler(ConsoleHandler, TRUE);
	app.Run();
	return 0;
}
