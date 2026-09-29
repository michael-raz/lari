pub use std::ops::{Add, AddAssign, Sub, Neg, Mul, Div};
use std::fmt::{self, Debug, Formatter};

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct Vec2<T=i64>
	where T: Copy,
{
	pub x: T,
	pub y: T,
}
impl<T> Vec2<T>
	where T: Copy,
{
	pub fn new(x: T, y: T) -> Self {
		Self{x, y}
	}
}


#[macro_export]
macro_rules! vec2 {
	($x:expr, $y:expr) => {
		Vec2::new($x, $y)
	};
}
pub use vec2;

impl<T> From<(T, T)> for Vec2<T>
	where T: Copy,
{
	fn from((x, y): (T, T)) -> Self {
		Self{x, y}
	}
}
impl<T> From<&(T, T)> for Vec2<T>
	where T: Copy,
{
	fn from((x, y): &(T, T)) -> Self {
		Self::from((*x, *y))
	}
}

impl<T> Debug for Vec2<T>
	where T: Copy + Debug,
{
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		write!(f, "[{:?} {:?}]", self.x, self.y)?;
		return Ok(());
	}
}

impl<T> Add<Self> for Vec2<T>
	where T: Copy + Add<T, Output=T>,
{
	type Output = Self;
	fn add(mut self, rhs: Self) -> Self::Output {
		self += rhs;
		self
	}
}
impl<T> AddAssign<Self> for Vec2<T>
	where T: Copy + Add<T, Output=T>,
{
	fn add_assign(&mut self, rhs: Self) {
		self.x = self.x + rhs.x;
		self.y = self.y + rhs.y;
	}
}
impl<T> Add<T> for Vec2<T>
	where T: Copy + Add<T, Output=T>,
{
	type Output = Self;
	fn add(mut self, rhs: T) -> Self::Output {
		self.x = self.x + rhs;
		self.y = self.y + rhs;
		self
	}
}
impl<T> Sub<Self> for Vec2<T>
	where T: Copy + Sub<T, Output=T>,
{
	type Output = Self;
	fn sub(mut self, rhs: Self) -> Self::Output {
		self.x = self.x - rhs.x;
		self.y = self.y - rhs.y;
		self
	}
}
impl<T> Sub<T> for Vec2<T>
	where T: Copy + Sub<T, Output=T>,
{
	type Output = Self;
	fn sub(mut self, rhs: T) -> Self::Output {
		self.x = self.x - rhs;
		self.y = self.y - rhs;
		self
	}
}

impl<T> Neg for Vec2<T>
	where T: Copy + Neg<Output=T>,
{
	type Output = Self;
	fn neg(mut self) -> Self::Output {
		self.x = -self.x;
		self.y = -self.y;
		self
	}
}

impl<T> Mul<T> for Vec2<T>
	where T: Copy + Mul<T, Output=T>,
{
	type Output = Self;
	fn mul(mut self, rhs: T) -> Self::Output {
		self.x = self.x * rhs;
		self.y = self.y * rhs;
		self
	}
}

impl<T> Div<T> for Vec2<T>
	where T: Copy + Div<T, Output=T>,
{
	type Output = Self;
	fn div(mut self, rhs: T) -> Self::Output {
		self.x = self.x / rhs;
		self.y = self.y / rhs;
		self
	}
}
