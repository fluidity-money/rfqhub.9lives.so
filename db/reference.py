#!/usr/bin/env python3

"""
Reference implementation of the database design, without the asset and signatures. The
calculation is actually done in the database here as well, but the end price must be
provided using an external input.
"""

class OnboardingAmtNotEnough(Exception):
	pass

class OnboardingAmounts:
	"""
	The amounts that were recorded last as having been onboarded.
	"""
	onboarded = {}

	"""
	Take the max that we can take from this user, if we're below the minimum we want
	to error out.
	"""
	def take_max(self, name, min, max_):
		avail = self.onboarded[name]
		if avail >= min:
			to_take = max(max_, avail)
			self.onboarded[name] -= to_take
			return to_take
		else:
			raise OnboardingAmtNotEnough()

class BundleTaker:
	"""
	Account address that's used to designate the owner.
	"""
	acc_addr = None

	"""
	The source of this liquidity. If this is set to None, then it must've come from
	the OnboardingSource. If it's set to something else, then it must've come from an
	aggregated bundle.
	"""
	source = None

	"""
	The amount that's being offered here to a BundleMaker, if they win.
	"""
	amt = 0

	"""
	Was this prediction up or down?
	"""
	is_up = False

	"""
	The target for the outcome here.
	"""
	price_target = 0

	"""
	Outcome that we're speculating on .
	"""
	outcome = "Bitcoin"

	"""
	The Maker this is associated with, if any.
	"""
	bundle_maker_id = None

	"""
	The end amount of this BundleTaker after the winner has been chosen. Based on a
	local calculation that we perform here.
	"""
	end_source_amt = 0

	def __init__(
		self,
		acc_addr,
		source,
		amt,
		is_up,
		price_target,
		outcome):
		self.acc_addr = acc_addr
		self.source = source
		self.amt = amt
		self.is_up = is_up
		self.price_target = price_target
		self.outcome = outcome

class BundleMaker:
	"""
	Account address that's used to designate the owner.
	"""
	acc_addr = None

	"""
	The source of this liquidity. If this is set to None, then it must've come from
	the OnboardingSource. If it's set to something else, then it must've come from an
	aggregated bundle.
	"""
	source = None

	"""
	The id of the BundleTaker that we're associated with here.
	"""
	bundle_taker_id = None

	"""
	The amount that's being offered to a BundleTaker if they win.
	"""
	amt = 0

	"""
	Was this chosen to be the bundle at the other side?
	"""
	was_chosen = None

	"""
	Was this correct? Unset until chosen.
	"""
	was_correct = None

	"""
	Stand-in for the Timestamplet that we use to communicate the end result of
	everything. Set later if this one is chosen.
	"""
	end_price_amt = None
	"""
	The end amount of this BundleMaker after the winner has been chosen. This lets us
	simply use this class again for a Aggregate calculation.
	"""
	end_source_amt = 0

	"""
	The amount elected to be returned to the pot after the user constructed
	AggregatedResultArgs with the amount being less than the end_source_amt.
	These are not considered to be in the pot, with the system preferring to rely
	on the event that's emitted instead, so this is purely informational.
	"""
	amt_to_onramping_amt = 0

	def __init__(
		self,
		acc_addr,
		source,
		bundle_taker_id,
		amt):
		self.acc_addr = acc_addr
		self.source = source
		self.bundle_taker_id = bundle_taker_id
		self.amt = amt

class Database:
	"""
	Any taker bundles that are in circulation.
	"""
	bundle_takers = []

	"""
	Any maker bundles that are in circulation.
	"""
	bundle_makers = []

	def add_bundle_taker(self, b):
		x = len(self.bundle_takers)
		self.bundle_takers.append(b)
		return x

	def add_bundle_maker(self, b):
		x = len(self.bundle_makers)
		self.bundle_makers.append(b)
		return x

	def pick_maker(self, bundle_taker_id, bundle_maker_id):
		self.bundle_takers[bundle_taker_id].bundle_maker = bundle_maker_id
		self.bundle_makers[bundle_maker_id].was_chosen = True
		for i in range(len(self.bundle_makers)):
			is_connected = self.bundle_makers[i].bundle_taker_id == bundle_taker_id
			if is_connected and bundle_maker_id != i:
				self.bundle_makers[i].was_chosen = False

	def set_aggregate_fields(
		self,
		onboarding_amts,
		bundle_taker_acc,
		bundle_maker_acc,
		bundle_taker_id,
		bundle_maker_id,
		end_price_amt,
		taker_to_leave_leftover,
		maker_to_leave_leftover):
		bundle_taker = self.bundle_takers[bundle_taker_id]
		bundle_maker = self.bundle_makers[bundle_maker_id]
		bundle_maker.end_price_amt = end_price_amt

		is_above_target = end_price_amt > bundle_taker.price_target
		is_taker_correct = (is_above_target and bundle_taker.is_up) or (not is_above_target and not bundle_taker.is_up)
		if is_taker_correct:
			"""
			A fundamental design flaw with the system we have is that it's all
			or nothing if you reinvest your winnings. A future system might
			let someone fork out what they have partially.
			"""
			# This has poor hygiene, if someone were to ask for more, this
			# would go negative. But this is a test:
			bundle_taker.end_source_amt = bundle_taker.amt + bundle_maker.amt - taker_to_leave_leftover
			bundle_maker.end_source_amt = 0
			o.onboarded[bundle_taker_acc] += taker_to_leave_leftover
		else:
			bundle_taker.end_source_amt = 0
			bundle_maker.end_source_amt = bundle_taker.amt + bundle_maker.amt - maker_to_leave_leftover
			o.onboarded[bundle_maker_acc] += maker_to_leave_leftover

	def mark_as_offramped(
		self,
		bundle_taker_id,
		bundle_maker_id):
		bundle_taker = self.bundle_takers[bundle_taker_id]
		bundle_maker = self.bundle_makers[bundle_maker_id]
		bundle_taker.was_offramped = True
		bundle_maker.was_offramped = True
		return (bundle_taker.end_source_amt, bundle_maker.end_source_amt)

	def get_taker_leftover(
		self,
		bundle_taker_id):
		return self.bundle_takers[bundle_taker_id].end_source_amt

if __name__ == "__main__":
	"""
	This is a translation of the test from TestRfqhub.t.sol that we've also used in
	the Rust code, but with a flow that captures the database interaction including
	our graph.
	"""
	db = Database()
	o = OnboardingAmounts()
	o.onboarded["Erik"] = 500
	o.onboarded["Alex"] = 200
	erik = BundleTaker(
		"Erik",					# The identifier for this bundle, we don't type in this example.
		None,					# None, so the source is onramping.
		o.take_max("Erik", 500, 500),	# Starting liquidity.
		True,					# Is up?
		100,						# Price target.
		"Bitcoin"					# The outcome we're speculating on.
	)
	bundle_taker_id = db.add_bundle_taker(erik)
	alex = BundleMaker(
		"Alex",					# The identifier.
		None,					# The source is onramping here as well.
		bundle_taker_id,			# The BundleTaker id that we refer to here.
		o.take_max("Alex", 200, 200)	# The amount that we're offering to Erik if he wins.
	)
	bundle_maker_id = db.add_bundle_maker(alex)
	"""
	Let's diverge from the test so far by adding a submitter that won't be chosen:
	"""
	db.add_bundle_maker(BundleMaker(
		"Shahmeer",
		None,
		bundle_taker_id,
		100				# Shahmeer wasn't chosen since his offer sucks.
	))
	db.pick_maker(bundle_taker_id, bundle_maker_id)
	"""
	We set the aggreate fields here for the result. This lets us reuse this soon.
	"""
	db.set_aggregate_fields(
		o,
		"Erik",
		"Alex",
		bundle_taker_id,
		bundle_maker_id,
		101,
		0,
		0
	)
	"""
	We ask the system for a signature in this example (which would actually produce
	the aggregate type her), so they can offramp:
	"""
	db.mark_as_offramped(bundle_taker_id, bundle_maker_id)
	"""
	This is the same test as above, except Erik chose to reinvest the money he made
	immediately after:
	"""
	o = OnboardingAmounts()
	o.onboarded["Erik"] = 500
	o.onboarded["Alex"] = 200
	erik = BundleTaker(
		"Erik",					# The identifier for this bundle, we don't type in this example.
		None,					# None, so the source is onramping.
		o.take_max("Erik", 500, 500),	# Starting liquidity.
		True,					# Is up?
		100,						# Price target.
		"Bitcoin"					# The outcome we're speculating on.
	)
	bundle_taker_id = db.add_bundle_taker(erik)
	alex = BundleMaker(
		"Alex",					# The identifier.
		None,					# The source is onramping here as well.
		bundle_taker_id,			# The BundleTaker id that we refer to here.
		o.take_max("Alex", 200, 200)	# The amount that we're offering to Erik if he wins.
	)
	bundle_maker_id = db.add_bundle_maker(alex)
	db.pick_maker(bundle_taker_id, bundle_maker_id)
	db.set_aggregate_fields(
		o,
		"Erik",
		"Alex",
		bundle_taker_id,
		bundle_maker_id,
		101,
		0,
		0
	)
	"""
	Unlike in the test this is based on, the taker here is going to reinvest the
	amount to bundle a new trade against Shahmeer.
	"""
	erik_available = db.get_taker_leftover(bundle_taker_id)
	erik = BundleTaker(
		"Erik",
		bundle_taker_id,
		# Though this is an argument here, this is always needing to be the amount
		# leftover from before:
		erik_available,
		False,
		200,
		"Bitcoin"
	)
	bundle_taker_id = db.add_bundle_taker(erik)
	o.onboarded["Shahmeer"] = 300
	shahmeer = BundleMaker(
		"Shahmeer",
		None,
		bundle_taker_id,
		o.take_max("Shahmeer", 300, 300)
	)
	bundle_maker_id = db.add_bundle_maker(shahmeer)
	db.pick_maker(bundle_taker_id, bundle_maker_id)
	"""
	Unlike in the first example, we're going to take about $200 from Erik's winning
	position:
	"""
	db.set_aggregate_fields(
		o,
		"Erik",
		"Alex",
		bundle_taker_id,
		bundle_maker_id,
		101,
		200,
		0
	)
	(erik_won, shahmeer_won) = db.mark_as_offramped(bundle_taker_id, bundle_maker_id)
	assert(erik_won == 800)
	assert(shahmeer_won == 0)
	assert(o.onboarded["Erik"] == 200)
