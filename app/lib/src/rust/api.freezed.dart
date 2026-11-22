// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint, type=warning, deprecated_member_use, deprecated_member_use_from_same_package
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'api.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// GENERATED CODE - DO NOT MODIFY BY HAND
// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$Msg {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is Msg);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'Msg()';
}


}

/// @nodoc
class $MsgCopyWith<$Res>  {
$MsgCopyWith(Msg _, $Res Function(Msg) __);
}


/// Adds pattern-matching-related methods to [Msg].
extension MsgPatterns on Msg {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( Msg_Status value)?  status,TResult Function( Msg_Request value)?  request,TResult Function( Msg_History value)?  history,TResult Function( Msg_Rules value)?  rules,TResult Function( Msg_Notice value)?  notice,TResult Function( Msg_Online value)?  online,TResult Function( Msg_Problem value)?  problem,required TResult orElse(),}){
final _that = this;
switch (_that) {
case Msg_Status() when status != null:
return status(_that);case Msg_Request() when request != null:
return request(_that);case Msg_History() when history != null:
return history(_that);case Msg_Rules() when rules != null:
return rules(_that);case Msg_Notice() when notice != null:
return notice(_that);case Msg_Online() when online != null:
return online(_that);case Msg_Problem() when problem != null:
return problem(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( Msg_Status value)  status,required TResult Function( Msg_Request value)  request,required TResult Function( Msg_History value)  history,required TResult Function( Msg_Rules value)  rules,required TResult Function( Msg_Notice value)  notice,required TResult Function( Msg_Online value)  online,required TResult Function( Msg_Problem value)  problem,}){
final _that = this;
switch (_that) {
case Msg_Status():
return status(_that);case Msg_Request():
return request(_that);case Msg_History():
return history(_that);case Msg_Rules():
return rules(_that);case Msg_Notice():
return notice(_that);case Msg_Online():
return online(_that);case Msg_Problem():
return problem(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( Msg_Status value)?  status,TResult? Function( Msg_Request value)?  request,TResult? Function( Msg_History value)?  history,TResult? Function( Msg_Rules value)?  rules,TResult? Function( Msg_Notice value)?  notice,TResult? Function( Msg_Online value)?  online,TResult? Function( Msg_Problem value)?  problem,}){
final _that = this;
switch (_that) {
case Msg_Status() when status != null:
return status(_that);case Msg_Request() when request != null:
return request(_that);case Msg_History() when history != null:
return history(_that);case Msg_Rules() when rules != null:
return rules(_that);case Msg_Notice() when notice != null:
return notice(_that);case Msg_Online() when online != null:
return online(_that);case Msg_Problem() when problem != null:
return problem(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( List<Kid> children)?  status,TResult Function( String id,  String user,  int minutes,  String reason)?  request,TResult Function( List<ChildHistory> children)?  history,TResult Function( List<Rules> children)?  rules,TResult Function( String text)?  notice,TResult Function()?  online,TResult Function( String text)?  problem,required TResult orElse(),}) {final _that = this;
switch (_that) {
case Msg_Status() when status != null:
return status(_that.children);case Msg_Request() when request != null:
return request(_that.id,_that.user,_that.minutes,_that.reason);case Msg_History() when history != null:
return history(_that.children);case Msg_Rules() when rules != null:
return rules(_that.children);case Msg_Notice() when notice != null:
return notice(_that.text);case Msg_Online() when online != null:
return online();case Msg_Problem() when problem != null:
return problem(_that.text);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( List<Kid> children)  status,required TResult Function( String id,  String user,  int minutes,  String reason)  request,required TResult Function( List<ChildHistory> children)  history,required TResult Function( List<Rules> children)  rules,required TResult Function( String text)  notice,required TResult Function()  online,required TResult Function( String text)  problem,}) {final _that = this;
switch (_that) {
case Msg_Status():
return status(_that.children);case Msg_Request():
return request(_that.id,_that.user,_that.minutes,_that.reason);case Msg_History():
return history(_that.children);case Msg_Rules():
return rules(_that.children);case Msg_Notice():
return notice(_that.text);case Msg_Online():
return online();case Msg_Problem():
return problem(_that.text);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( List<Kid> children)?  status,TResult? Function( String id,  String user,  int minutes,  String reason)?  request,TResult? Function( List<ChildHistory> children)?  history,TResult? Function( List<Rules> children)?  rules,TResult? Function( String text)?  notice,TResult? Function()?  online,TResult? Function( String text)?  problem,}) {final _that = this;
switch (_that) {
case Msg_Status() when status != null:
return status(_that.children);case Msg_Request() when request != null:
return request(_that.id,_that.user,_that.minutes,_that.reason);case Msg_History() when history != null:
return history(_that.children);case Msg_Rules() when rules != null:
return rules(_that.children);case Msg_Notice() when notice != null:
return notice(_that.text);case Msg_Online() when online != null:
return online();case Msg_Problem() when problem != null:
return problem(_that.text);case _:
  return null;

}
}

}

/// @nodoc


class Msg_Status extends Msg {
  const Msg_Status({required  List<Kid> children}): _children = children,super._();
  

 final  List<Kid> _children;
 List<Kid> get children {
  if (_children is EqualUnmodifiableListView) return _children;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_children);
}


/// Create a copy of Msg
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$Msg_StatusCopyWith<Msg_Status> get copyWith => _$Msg_StatusCopyWithImpl<Msg_Status>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is Msg_Status&&const DeepCollectionEquality().equals(other.children, _children));
}


@override
int get hashCode {
    return Object.hash(runtimeType,const DeepCollectionEquality().hash(_children));
}

@override
String toString() {
    return 'Msg.status(children: $children)';
}


}

/// @nodoc
abstract mixin class $Msg_StatusCopyWith<$Res> implements $MsgCopyWith<$Res> {
  factory $Msg_StatusCopyWith(Msg_Status value, $Res Function(Msg_Status) _then) = _$Msg_StatusCopyWithImpl;
@useResult
$Res call({
 List<Kid> children
});




}
/// @nodoc
class _$Msg_StatusCopyWithImpl<$Res>
    implements $Msg_StatusCopyWith<$Res> {
  _$Msg_StatusCopyWithImpl(this._self, this._then);

  final Msg_Status _self;
  final $Res Function(Msg_Status) _then;

/// Create a copy of Msg
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? children = null,}) {
  return _then(Msg_Status(
children: null == children ? _self._children : children // ignore: cast_nullable_to_non_nullable
as List<Kid>,
  ));
}


}

/// @nodoc


class Msg_Request extends Msg {
  const Msg_Request({required this.id, required this.user, required this.minutes, required this.reason}): super._();
  

 final  String id;
 final  String user;
 final  int minutes;
 final  String reason;

/// Create a copy of Msg
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$Msg_RequestCopyWith<Msg_Request> get copyWith => _$Msg_RequestCopyWithImpl<Msg_Request>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is Msg_Request&&(identical(other.id, id) || other.id == id)&&(identical(other.user, user) || other.user == user)&&(identical(other.minutes, minutes) || other.minutes == minutes)&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode {
    return Object.hash(runtimeType,id,user,minutes,reason);
}

@override
String toString() {
    return 'Msg.request(id: $id, user: $user, minutes: $minutes, reason: $reason)';
}


}

/// @nodoc
abstract mixin class $Msg_RequestCopyWith<$Res> implements $MsgCopyWith<$Res> {
  factory $Msg_RequestCopyWith(Msg_Request value, $Res Function(Msg_Request) _then) = _$Msg_RequestCopyWithImpl;
@useResult
$Res call({
 String id, String user, int minutes, String reason
});




}
/// @nodoc
class _$Msg_RequestCopyWithImpl<$Res>
    implements $Msg_RequestCopyWith<$Res> {
  _$Msg_RequestCopyWithImpl(this._self, this._then);

  final Msg_Request _self;
  final $Res Function(Msg_Request) _then;

/// Create a copy of Msg
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? id = null,Object? user = null,Object? minutes = null,Object? reason = null,}) {
  return _then(Msg_Request(
id: null == id ? _self.id : id // ignore: cast_nullable_to_non_nullable
as String,user: null == user ? _self.user : user // ignore: cast_nullable_to_non_nullable
as String,minutes: null == minutes ? _self.minutes : minutes // ignore: cast_nullable_to_non_nullable
as int,reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class Msg_History extends Msg {
  const Msg_History({required  List<ChildHistory> children}): _children = children,super._();
  

 final  List<ChildHistory> _children;
 List<ChildHistory> get children {
  if (_children is EqualUnmodifiableListView) return _children;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_children);
}


/// Create a copy of Msg
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$Msg_HistoryCopyWith<Msg_History> get copyWith => _$Msg_HistoryCopyWithImpl<Msg_History>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is Msg_History&&const DeepCollectionEquality().equals(other.children, _children));
}


@override
int get hashCode {
    return Object.hash(runtimeType,const DeepCollectionEquality().hash(_children));
}

@override
String toString() {
    return 'Msg.history(children: $children)';
}


}

/// @nodoc
abstract mixin class $Msg_HistoryCopyWith<$Res> implements $MsgCopyWith<$Res> {
  factory $Msg_HistoryCopyWith(Msg_History value, $Res Function(Msg_History) _then) = _$Msg_HistoryCopyWithImpl;
@useResult
$Res call({
 List<ChildHistory> children
});




}
/// @nodoc
class _$Msg_HistoryCopyWithImpl<$Res>
    implements $Msg_HistoryCopyWith<$Res> {
  _$Msg_HistoryCopyWithImpl(this._self, this._then);

  final Msg_History _self;
  final $Res Function(Msg_History) _then;

/// Create a copy of Msg
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? children = null,}) {
  return _then(Msg_History(
children: null == children ? _self._children : children // ignore: cast_nullable_to_non_nullable
as List<ChildHistory>,
  ));
}


}

/// @nodoc


class Msg_Rules extends Msg {
  const Msg_Rules({required  List<Rules> children}): _children = children,super._();
  

 final  List<Rules> _children;
 List<Rules> get children {
  if (_children is EqualUnmodifiableListView) return _children;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_children);
}


/// Create a copy of Msg
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$Msg_RulesCopyWith<Msg_Rules> get copyWith => _$Msg_RulesCopyWithImpl<Msg_Rules>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is Msg_Rules&&const DeepCollectionEquality().equals(other.children, _children));
}


@override
int get hashCode {
    return Object.hash(runtimeType,const DeepCollectionEquality().hash(_children));
}

@override
String toString() {
    return 'Msg.rules(children: $children)';
}


}

/// @nodoc
abstract mixin class $Msg_RulesCopyWith<$Res> implements $MsgCopyWith<$Res> {
  factory $Msg_RulesCopyWith(Msg_Rules value, $Res Function(Msg_Rules) _then) = _$Msg_RulesCopyWithImpl;
@useResult
$Res call({
 List<Rules> children
});




}
/// @nodoc
class _$Msg_RulesCopyWithImpl<$Res>
    implements $Msg_RulesCopyWith<$Res> {
  _$Msg_RulesCopyWithImpl(this._self, this._then);

  final Msg_Rules _self;
  final $Res Function(Msg_Rules) _then;

/// Create a copy of Msg
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? children = null,}) {
  return _then(Msg_Rules(
children: null == children ? _self._children : children // ignore: cast_nullable_to_non_nullable
as List<Rules>,
  ));
}


}

/// @nodoc


class Msg_Notice extends Msg {
  const Msg_Notice({required this.text}): super._();
  

 final  String text;

/// Create a copy of Msg
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$Msg_NoticeCopyWith<Msg_Notice> get copyWith => _$Msg_NoticeCopyWithImpl<Msg_Notice>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is Msg_Notice&&(identical(other.text, text) || other.text == text));
}


@override
int get hashCode {
    return Object.hash(runtimeType,text);
}

@override
String toString() {
    return 'Msg.notice(text: $text)';
}


}

/// @nodoc
abstract mixin class $Msg_NoticeCopyWith<$Res> implements $MsgCopyWith<$Res> {
  factory $Msg_NoticeCopyWith(Msg_Notice value, $Res Function(Msg_Notice) _then) = _$Msg_NoticeCopyWithImpl;
@useResult
$Res call({
 String text
});




}
/// @nodoc
class _$Msg_NoticeCopyWithImpl<$Res>
    implements $Msg_NoticeCopyWith<$Res> {
  _$Msg_NoticeCopyWithImpl(this._self, this._then);

  final Msg_Notice _self;
  final $Res Function(Msg_Notice) _then;

/// Create a copy of Msg
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? text = null,}) {
  return _then(Msg_Notice(
text: null == text ? _self.text : text // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class Msg_Online extends Msg {
  const Msg_Online(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is Msg_Online);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'Msg.online()';
}


}




/// @nodoc


class Msg_Problem extends Msg {
  const Msg_Problem({required this.text}): super._();
  

 final  String text;

/// Create a copy of Msg
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$Msg_ProblemCopyWith<Msg_Problem> get copyWith => _$Msg_ProblemCopyWithImpl<Msg_Problem>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is Msg_Problem&&(identical(other.text, text) || other.text == text));
}


@override
int get hashCode {
    return Object.hash(runtimeType,text);
}

@override
String toString() {
    return 'Msg.problem(text: $text)';
}


}

/// @nodoc
abstract mixin class $Msg_ProblemCopyWith<$Res> implements $MsgCopyWith<$Res> {
  factory $Msg_ProblemCopyWith(Msg_Problem value, $Res Function(Msg_Problem) _then) = _$Msg_ProblemCopyWithImpl;
@useResult
$Res call({
 String text
});




}
/// @nodoc
class _$Msg_ProblemCopyWithImpl<$Res>
    implements $Msg_ProblemCopyWith<$Res> {
  _$Msg_ProblemCopyWithImpl(this._self, this._then);

  final Msg_Problem _self;
  final $Res Function(Msg_Problem) _then;

/// Create a copy of Msg
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? text = null,}) {
  return _then(Msg_Problem(
text: null == text ? _self.text : text // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

// dart format on
