#pragma once

#include <utility>

namespace attr_test_ns {

// ---------------------------------------------------------------------------
// Case 1: constexpr (standard C++ specifier — garbles signature on own line)
// ---------------------------------------------------------------------------

template <typename T>
struct ConstexprWidget {
  constexpr ConstexprWidget();
  constexpr ConstexprWidget(const ConstexprWidget &other);
  constexpr ConstexprWidget(ConstexprWidget &&other);
  ~ConstexprWidget();
  T value{};
};

// clang-format off
template <typename T>
constexpr
ConstexprWidget<T>::ConstexprWidget() {}

template <typename T>
constexpr
ConstexprWidget<T>::ConstexprWidget(const ConstexprWidget &other)
    : value(other.value) {}

template <typename T>
constexpr
ConstexprWidget<T>::ConstexprWidget(ConstexprWidget &&other)
    : value(std::move(other.value)) {}
// clang-format on

template <typename T>
ConstexprWidget<T>::~ConstexprWidget() {}

// ---------------------------------------------------------------------------
// Case 2: [[nodiscard]] (C++ attribute — garbles signature field)
// ---------------------------------------------------------------------------

template <typename T>
struct NodiscardWidget {
  [[nodiscard]] NodiscardWidget();
  [[nodiscard]] NodiscardWidget(const NodiscardWidget &other);
  [[nodiscard]] NodiscardWidget(NodiscardWidget &&other);
  ~NodiscardWidget();
  T value{};
};

// clang-format off
template <typename T>
[[nodiscard]]
NodiscardWidget<T>::NodiscardWidget() {}

template <typename T>
[[nodiscard]]
NodiscardWidget<T>::NodiscardWidget(const NodiscardWidget &other)
    : value(other.value) {}

template <typename T>
[[nodiscard]]
NodiscardWidget<T>::NodiscardWidget(NodiscardWidget &&other)
    : value(std::move(other.value)) {}
// clang-format on

template <typename T>
NodiscardWidget<T>::~NodiscardWidget() {}

// ---------------------------------------------------------------------------
// Case 3: _CUSTOM_ATTRIBUTE (used for conditional or compiler-extension project-specific attributes)
// ---------------------------------------------------------------------------

#define _CUSTOM_ATTRIBUTE

template <typename T>
struct CustomWidget {
  _CUSTOM_ATTRIBUTE CustomWidget();
  _CUSTOM_ATTRIBUTE CustomWidget(const CustomWidget &other);
  _CUSTOM_ATTRIBUTE CustomWidget(CustomWidget &&other);
  ~CustomWidget();
  T value{};
};

template <typename T>
_CUSTOM_ATTRIBUTE
CustomWidget<T>::CustomWidget() {}

template <typename T>
_CUSTOM_ATTRIBUTE
CustomWidget<T>::CustomWidget(const CustomWidget &other)
    : value(other.value) {}

template <typename T>
_CUSTOM_ATTRIBUTE
CustomWidget<T>::CustomWidget(CustomWidget &&other)
    : value(std::move(other.value)) {}

template <typename T>
CustomWidget<T>::~CustomWidget() {}

} // namespace attr_test_ns
