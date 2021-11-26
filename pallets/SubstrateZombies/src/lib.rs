//* Substrate Zombies */

#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>
pub use pallet::*;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
use scale_info::TypeInfo;
use frame_support::{
	dispatch::DispatchResult,
	pallet_prelude::*,
	traits::{Currency, Randomness, LockableCurrency, Time, UnixTime, ReservableCurrency},
};
use codec::HasCompact;
use frame_system::pallet_prelude::*;
use sp_io::hashing::blake2_128;
use sp_runtime::traits::{AtLeast32BitUnsigned, Bounded, StaticLookup, Printable};
//use sp_runtime::serde::{Desberialize, Serialize};

use codec::FullCodec;
use frame_support::traits::ExistenceRequirement;
use frame_support::traits::WithdrawReasons;
use frame_system::ensure_signed;
use frame_system::pallet_prelude::*;
use sp_runtime::{ArithmeticError, print};


#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug, serde::Serialize, serde::Deserialize))]
pub enum Gender {
    Male,
    Female,
}

//* Custom Type */
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug, serde::Serialize, serde::Deserialize))]
pub struct ZombieDNA<T: Config> { 
	pub dna: Option<Vec<u8>>,
	pub owner: T::AccountId,
	pub for_sale: bool,
	pub head: Vec<u8>,
	pub eye: Vec<u8>,
	pub shirt_gene: Vec<u8>,
	pub eye_colour_gene: Vec<u8>,
	pub clothes_colours_gene: Vec<u8>,
	pub gender: Gender
}
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug, serde::Serialize, serde::Deserialize))]
pub struct SellOrder<T: Config, AccountIdOf, BalanceOf> { 
	pub item_id: T::ZombieIndex, 
	pub owner: AccountIdOf,
	pub price: Option<BalanceOf>,
	pub created_at: T::Timestamp
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	
	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::generate_storage_info]
	pub struct Pallet<T>(_);
	
	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config  {
		
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		
		//	Currency used to faciliate buy and sell of Zombies
		type Currency: Currency<AccountIdOf<Self>> + LockableCurrency<AccountIdOf<Self>, Moment=Self::BlockNumber> + Send + Sync;
		
		//	type id for identifying Zombies
		type ZombieIndex: Parameter + Member + AtLeast32BitUnsigned + Default + Copy + MaybeSerializeDeserialize + Bounded + FullCodec;
		
		//	Used to generate Random DNA 
		type Randomness: Randomness<Self::Hash, Self::BlockNumber>;
		
		//	The origin which may forcibly create or destroy an asset or otherwise alter priviledged attributes
		type ForceOrigin: EnsureOrigin<Self::Origin>;
		
		//	Get the time 
		type Timestamp: Time;
		
		//	The maximum length of an attribute value 
		#[pallet::constant]
		type MaxValueLimit: Get<u8>;
	}
	type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	type BalanceOf<T> = <<T as Config>::Currency as Currency<AccountIdOf<T>>>::Balance;
	//	Data Structure for Zombie Dna 
	type ZombieInfoOf<T> = ZombieDNA<AccountIdOf<T>>;
	pub type CurrencyOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	type SellInfoOf<T> = SellOrder<T: Config, AccountIdOf<T>, BalanceOf<T>>;
	
//* --Storage Items-- */
	//	Storage Value for ZombieIndex 
	#[pallet::storage]
	#[pallet::getter(fn zombie_id)]
	pub type ZombieId<T> = StorageValue<_, T::ZombieIndex, ValueQuery>;

	//	Account associated with ZombieIndex and Zombie DNA
	#[pallet::storage]
	#[pallet::getter(fn zombies)]
	pub type Zombies<T> = StorageDoubleMap<
		_,
		Blake2_128Concat, T::ZombieIndex,
		Blake2_128Concat, AccountIdOf<T>,
		ZombieInfoOf<T>,
		ValueQuery,
	>;
	// 	The storage that handles all sell orders 
	#[pallet::storage]
	#[pallet::getter(fn zombie_price)]
	pub type SellOrderInfo<T> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::ZombieIndex,
		Blake2_128Concat,
		AccountIdOf<T>,
		SellInfoOf<T>,
		OptionQuery,
	>;
	
//* --Events-- */
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		NewZombieCreated { 
			owner: T::AccountId,
			zombie_id: T::ZombieIndex, 
			gender: Gender
		},
		Transferred { 
			owner: T::AccountId, 
			to: T::AccountId, 
			zombie_id: T::ZombieIndex,
		},
		Destroyed { 
			owner: T::AccountId,
			index: T::ZombieIndex, 
		},
		CreatedOrder  {
			zombie_id: T::ZombieIndex,
			owner: T::AccountId, 
		},
		CancelledOrder { 
			id: T::ZombieIndex, 
			time: T::Timestamp,
		},
		OrderTaken { 
			zombie_id: T::ZombieIndex, 
			bought_at: T::Timestamp,
		}
	}
//* --Errors-- */
	#[pallet::error]
	pub enum Error<T> {
		SameOwner,
		ZombieNotFound,
		InvalidZombieId,
		Unknown,
		UnAuthorised,
		SellOrderDoesNotExists,
	}	
	//	
	//	Allow User to create their custom Zombie Type!
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		pub fn create(
			origin: OriginFor<T>,
				#[pallet::compact] head: Vec<u8>,
				#[pallet::compact] eye: Vec<u8>,
				#[pallet::compact] shirt_gene: Vec<u8>,
				#[pallet::compact] skin_colour: Vec<u8>,
				#[pallet::compact] eye_colour_gene: Vec<u8>,
				#[pallet::compact] clothes_colours_gene: Vec<u8>,
				#[pallet::compact] gender: Option<Gender>

		) -> DispatchResultWithPostInfo { 
			let creator = ensure_signed(origin)?;
			
			let rand_dna = Self::gen_dna(&creator);
			let rand_gender = Self::gen_gender();
			let zombie_dna = Self::custom_dna(
				&head,
				&eye, 
				&shirt_gene,
				&skin_colour,
				&eye_colour_gene,
				&clothes_colours_gene
			);
			let zombie_id = Self::next_zombie_id()?;
			
			let zombie_info = ZombieDNA::<T> { 
				dna: zombie_dna.clone().unwrap_or_else(rand_dna),
				owner: creator.clone(),
				for_sale: false,
				head,
				eye,
				shirt_gene,
				eye_colour_gene,
				clothes_colours_gene,
				gender: gender.unwrap_or_else(rand_gender)
			};

			Zombies::<T>::add_zombie_to_account(&creator, &zombie_id, zombie_info);
			
			Self::deposit_event(Event::NewZombieCreated { 
				owner: creator,
				zombie_id,
				gender
			});

			Ok(().into())
		}
		#[pallet::weight(10_000)]
		pub fn transfer(
			origin: OriginFor<T>,
			#[pallet::compact] from: T::AccountId, 
			#[pallet::compact] to: T::AccountId,
			#[pallet::compact] zombie_id: T::ZombieIndex
		) -> DispatchResult { 
			let sender = ensure_signed(origin)?;
			ensure!(sender != to, Error::<T>::SameOwner);
			
			Zombies::<T>::transfer_zombie_ownership(sender, to, zombie_id);

			Ok(())
		}
		#[pallet::weight(10_000)]
		pub fn destroy(
			origin: OriginFor<T>,
			#[pallet::compact] owner: AccountIdOf<T>,
			#[pallet::compact] zombie_id: T::ZombieIndex,
			check_owner: Option<<T::Lookup as StaticLookup>::Source>,
		) -> DispatchResultWithPostInfo { 
			
			let maybe_check_owner = match T::ForceOrigin::try_origin(origin) {
				Ok(_) => None,
				Err(origin) => Some(ensure_signed(origin)?),
			};
			ZombieId::do_destory(owner, zombie_id, Event::Destroyed	{
				owner: maybe_check_owner.clone(),
				index: zombie_id,
			},)?;

			Ok(().into())
		}
		#[pallet::weight(10_000)]
		pub fn set_price(
			origin: OriginFor<T>,
				#[pallet::compact] price: BalanceOf<T>, 
				#[pallet::compact] zombie_id: T::ZombieIndex, 
				#[pallet::compact] owner: AccountIdOf<T>
		) -> DispatchResult  {
			let sender = ensure_signed(origin)?;

			//	check if the id exists
			Self::exists(zombie_id)?;
			ensure!(Zombies::<T>::contains_key(zombie_id, owner), Error::<T>::Unknown);
			//	check if the owner owns this id 
			let now = T::Timestamp::now();
			let order = SellOrder { 
				item_id: zombie_id.clone(),
				owner: sender.clone(),
				price,
				created_at: now.clone(),
			};
			//	Update "For Sale" Attribute from ZombieDNA 
			Zombies::<T>::try_mutate(zombie_id, owner, |info| -> DispatchResult { 
				let info = info.as_mut().ok_or(Error::<T>::ZombieNotFound)?;
				info.for_sale = true;

				Ok(())
			});

			SellOrderInfo::<T>::insert(zombie_id, sender, order);
			Self::deposit_event(Event::CreatedOrder { 
				zombie_id,
				owner: sender,
			});

		
			Ok(().into())
		}
		#[pallet::weight(10_000)]
		pub fn cancel_sell_order(
			origin: OriginFor<T>,
				#[pallet::compact] zombie_id: T::ZombieIndex, 
				#[pallet::compact] owner: AccountIdOf<T>
		) -> DispatchResult { 
			let sender = ensure_signed(origin)?;
			//	Check if the sell order exist
			ensure!(owner == sender, Error::<T>::UnAuthorised);
			ensure!(SellOrderInfo::<T>::contains_key(zombie_id, owner), Error::<T>::SellOrderDoesNotExists);
			
			let now = T::Timestamp::now();
			
			SellOrderInfo::<T>::remove(zombie_id, owner);
			Self::deposit_event(Event::<T>::CancelledOrder { 
				id: zombie_id, 
				time: now.clone(),
			});

			//	Update "For Sale" Attribute from ZombieDNA 
			Zombies::<T>::try_mutate(zombie_id, owner, |info| -> DispatchResult { 
				let info = info.as_mut().ok_or(Error::<T>::ZombieNotFound)?;
				info.for_sale = false;

				Ok(())
			});

			Ok(())
		}
		#[pallet::weight(10_000)]
		pub fn take_order(
			origin: OriginFor<T>,
				#[pallet::compact] zombie_id: T::ZombieIndex, 
				#[pallet::compact] owner: AccountIdOf<T>
		) -> DispatchResult { 
			let sender = ensure_signed(origin)?;
			ensure!(SellOrderInfo::<T>::contains_key(zombie_id, owner), Error::<T>::SellOrderDoesNotExists);
			let now = T::Timestamp::now();

			SellOrderInfo::<T>::take_order(zombie_id, sender, Event::OrderTaken { 
				zombie_id, 
				bought_at: now.clone(),
			})?;
			Ok(().into())
		}
		#[pallet::weight(10_000)]
		pub fn update_order_price(
			origin: OriginFor<T>,
				#[pallet::compact] zombie_id: T::ZombieIndex, 
				#[pallet::compact] price: BalanceOf<T>,
		) ->  DispatchResult { 
			let sender = ensure_signed(origin)?;

			//	Update Price order 

			Ok(())
		}
	}
// Helper Functions	
	impl<T: Config> Pallet<T> { 
		fn next_zombie_id() -> Result<T::ZombieIndex, DispatchError> { 
			ZombieId::<T>::try_mutate(|id| -> DispatchResult<T::ZombieIndex, Error> { 
				let current_id = *id;
				*id = id.checked_add(1).ok_or(ArithmeticError::Overflow)?;
				Ok(current_id)
			})
		}
		//	 Can be used to generate random dna 
		fn gen_dna(acc: &T::AccountId) -> Vec<u8> { 
			let payload = (
				T::Randomness::random(&b"dna"[..]).0,
				frame_system::Pallet::<T>::block_number(),
			);
			let mut dna = payload.using_encoded(blake2_128);
			dna.into_vec();
			dna
		}
		//	Check if the zombie exists 
		fn exists(id: T::ZombieIndex) -> bool { 
			let zombie_id = ZombieId::<T>::get();
			if zombie_id.binary_search(&id).is_ok() { true } else { 
				Error::<T>::InvalidZombieId;
				false
			}
		}
		fn custom_dna(
			head: &mut Vec<u8>,
			eye: &mut Vec<u8>,
			shirt_gene: &mut Vec<u8>,
			skin_colour: &mut Vec<u8>,
			eye_colour_gene: &mut Vec<u8>,
			clothes_colours_gene: &mut Vec<u8>,	
		) -> Vec<u8> { 
			let mut new_dna = head.iter(|| { 
				[head, eye, shirt_gene, skin_colour, eye_colour_gene, clothes_colours_gene].concat()
			});

			new_dna
		}
		//	Generate Gender
		fn gen_gender() -> Gender {
			let random = T::Randomness::random(&b"gender"[..]).0;
			match random.as_ref()[0] % 2 {
				0 => Gender::Male,
				_ => Gender::Female,
			}
		}

	}

	//	Storage Imples 
	//	Zombies Storage Handles "ZombieId" to "AccountID" for "ZombieInfo" 
	impl<T: Config> Zombies<T> { 
		fn add_zombie_to_account(acc: &T::AccountId, index: &T::ZombieIndex, zombie: ZombieInfoOf<T>) { 
			Zombies::<T>::insert(acc, index, zombie)
		}
		fn transfer_zombie_ownership( from: &T::AccountId, to: &T::AccountId, zombie_id: T::ZombieIndex)  { 
			Zombies::<T>::try_mutate_exists(from.clone(), to, |zombie| -> DispatchResult { 
				if from == to  {
					ensure!(zombie.is_some(), Error::<T>::InvalidZombieId);
					return Ok(());
				}
				let zombie = zombie.take().ok_or(Error::<T>::InvalidZombieId)?;
				Zombies::<T>::insert(&to, zombie_id, zombie);
				Self::deposit_event(Event::Transferred { 
					owner: from,
					to,
					zombie_id: zombie,
				});
				Ok(())
			})
		}
		fn update_details() { 
			//	TODO
		}
	}
	//	The Zombie Id stores ZombieIndex Ids
	impl<T: Config> ZombieId<T> { 
		fn do_destroy(maybe_owner: Option<T::AccountId>, index: T::ZombieIndex, event: Event<T>) -> DispatchResult { 
			ZombieId::<T>::try_mutate(|index| -> DispatchResult { 
				let info = index.take().ok_or(Error::<T>::ZombieNotFound)?;
				ZombieId::<T>::remove(&index);

				Ok(())
			})?;
			Self::deposit_event(event);

			Ok(())
		}
	}
	//	SellOrderInfo allows us to create and take sell orders 
	impl<T: Config> SellOrderInfo<T> { 
		fn create_sell_order() -> DispatchResult { 
			//	TODO
		}
		fn take_order(zombie_id: T::ZombieIndex, owner: AccountIdOf<T>, event: Event<T>) -> DispatchResult { 
			let target_sell_order = SellOrderInfo::<T>::get(zombie_id, owner);
			let owner = target_sell_order.owner;
			let price = target_sell_order.price;
			let price_of = CurrencyOf::<T>::saturated_from(price.into()); 

			let neg_imbalance = T::Currency::withdraw(
				&owner,
				price_of,
				WithdrawReasons::all(),
				ExistenceRequirement::AllowDeath,
			)?;
			T::Currency::resolve_creating(&owner, neg_imbalance);
			Self::deposit_event(event);			
			SellOrderInfo::<T>::remove(zombie_id, owner);

			Ok(())
		} 
	}
	//	Error Handling Methods 
	impl<T: Config> Printable for Pallet<T> {
		fn print(&self) { 
			match self { 
				Error::SameOwner => "You are transferring from the Same Owner!".print(),
				Error::ZombieNotFound => "We can find the Zombie you wanted, try again please!".print(),
				Error::Unknown => "Either you are unauthorised to perfom this action or you broke the whole thing".print(),
				Error::UnAuthorised => "No key id found, and it is not yours too ;D".print(),
				Error::SellOrderDoesNotExists => "Sell order not found".print(),
				Error::InvalidZombieId => "Wrong Id".print(),
			}
		}
	}
}


